"""Independently inspect native #14 evidence with MuPDF and pypdf.

Reads the native example's existing PDF/layout/engine-result artifacts. Writes
only an aggregate JSON report; does not alter PDFs or run the native shaper.
"""
import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path

import fitz
from pypdf import PdfReader
from pypdf.generic import ContentStream

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--input", type=Path, default=ROOT / "artifacts/shaped-text/native")
parser.add_argument("--output", type=Path, default=ROOT / "artifacts/shaped-text/native-independent.json")
parser.add_argument("--api-pdf", type=Path, default=ROOT / "artifacts/shaped-text/pdfium-api.pdf")
parser.add_argument("--check-invariants", action="store_true", help="Fail on serialization invariant errors; reader incompatibility remains recorded evidence")
args = parser.parse_args()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def inspect(path):
    reader = PdfReader(path)
    pages = []
    with fitz.open(path) as mupdf:
        for i, page in enumerate(reader.pages):
            fonts = []
            def embedded(resources_dict, depth=0):
                assert depth < 16
                obj = resources_dict.get_object()
                for name, value in obj.get("/Font", {}).items():
                    font = value.get_object()
                    descendant = font["/DescendantFonts"][0].get_object() if "/DescendantFonts" in font else font
                    descriptor = descendant.get("/FontDescriptor")
                    if descriptor and "/FontFile2" in descriptor.get_object():
                        data = descriptor.get_object()["/FontFile2"].get_data()
                        fonts.append(dict(resource=name, sha256=digest(data), bytes=len(data)))
                for value in obj.get("/XObject", {}).values():
                    child = value.get_object()
                    if "/Resources" in child:
                        embedded(child["/Resources"], depth + 1)
            embedded(page["/Resources"])
            pixmap = mupdf[i].get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False)
            pages.append(dict(mupdf=mupdf[i].get_text(), pypdf=page.extract_text(),
                              raster_sha256=digest(pixmap.samples), raster_size=[pixmap.width, pixmap.height],
                              fonts=fonts))
    return dict(file=str(path.resolve()), sha256=digest(path.read_bytes()), pages=pages)


native = json.loads((args.input / "results.json").read_text(encoding="utf-8"))
cases = []
for evidence in native["cases"]:
    source = args.input / evidence["pdf"]
    stem = source.stem
    layout = json.loads((args.input / f"{stem}.layout.json").read_text(encoding="utf-8"))
    original = inspect(source)
    resaved = inspect(args.input / f"{stem}.resaved.pdf")
    glyphs = [g for run in layout["runs"] for g in run["glyphs"]]
    reader = PdfReader(source)
    page = reader.pages[0]
    operations = ContentStream(page.get_contents(), reader).operations
    matrices = [list(map(float, values)) for values, operator in operations if operator == b"Tm"]
    draws = [values[0].original_bytes for values, operator in operations if operator == b"Tj"]
    font = next(iter(page["/Resources"]["/Font"].values())).get_object()
    mapping = font["/DescendantFonts"][0].get_object()["/CIDToGIDMap"].get_data()
    bounds = layout["bounds"]
    origin = [48 - min(0, bounds["xMin"]), 48 - min(0, bounds["yMin"])]
    errors = []
    for g, matrix in zip(glyphs, matrices):
        errors.extend([abs(matrix[4] - origin[0] - g["x"]), abs(matrix[5] - origin[1] - g["y"])])
    cid_ids = [int.from_bytes(draw, "big") for draw in draws]
    cid_indices_valid = all(len(draw) == 2 for draw in draws) and all(0 < cid and cid * 2 + 2 <= len(mapping) for cid in cid_ids)
    mapped_glyphs = [int.from_bytes(mapping[cid * 2:cid * 2 + 2], "big") for cid in cid_ids]
    def exact(reader, variant):
        # Only trailing CR/LF reader terminators are ignored; retain all internal
        # spaces, directional controls, normalization distinctions, and ordering.
        return variant["pages"][0][reader].rstrip("\r\n") == layout["text"]
    cases.append(dict(case=evidence["case"], rotation=evidence["rotation"], source=layout["text"],
                      original=original, resaved=resaved,
                      mupdf_exact=exact("mupdf", original), pypdf_exact=exact("pypdf", original),
                      mupdf_resaved_exact=exact("mupdf", resaved), pypdf_resaved_exact=exact("pypdf", resaved),
                      rasters_unchanged=original["pages"][0]["raster_sha256"] == resaved["pages"][0]["raster_sha256"],
                      text_unchanged=all(original["pages"][0][r] == resaved["pages"][0][r] for r in ("mupdf", "pypdf")),
                      font_bytes_match=all(variant["pages"][0]["fonts"] and all(f["sha256"] == layout["fontId"] for f in variant["pages"][0]["fonts"]) for variant in (original, resaved)),
                      glyph_count=len(glyphs), matrix_count=len(matrices), draw_count=len(draws),
                      glyph_matrix_draw_counts_match=0 < len(glyphs) == len(matrices) == len(draws),
                      allocated_cids_match=cid_ids == list(range(1, len(glyphs) + 1)),
                      cid_indices_valid=cid_indices_valid,
                      cid_mapping_matches=mapped_glyphs == [g["glyphId"] for g in glyphs],
                      max_position_error_points=max(errors, default=0),
                      matrix_linear_parts_match=all(m[:4] == [1, 0, 0, 1] for m in matrices)))
for case in cases:
    case["representation_invariants_pass"] = all(case[key] for key in (
        "font_bytes_match", "glyph_matrix_draw_counts_match", "allocated_cids_match",
        "cid_indices_valid", "cid_mapping_matches", "matrix_linear_parts_match",
        "rasters_unchanged", "text_unchanged")) and case["max_position_error_points"] <= 0.0001
result = dict(versions={name: importlib.metadata.version(name) for name in ("pymupdf", "pypdf")},
              native_shaper="HarfRust0.13.3 (upstream HarfBuzz14.3.1); these are native outputs, not Python shaping",
              normalization="Exact logical strings except trailing CR/LF reader terminators; raw strings retained",
              representation_invariants_pass=bool(cases) and all(case["representation_invariants_pass"] for case in cases),
              cases=cases, api=inspect(args.api_pdf) if args.api_pdf.exists() else None)
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(result, ensure_ascii=True, indent=2), encoding="utf-8")
for key in ("mupdf_exact", "pypdf_exact", "mupdf_resaved_exact", "pypdf_resaved_exact", "rasters_unchanged", "text_unchanged", "font_bytes_match", "cid_mapping_matches", "matrix_linear_parts_match", "representation_invariants_pass"):
    print(f"{key}: {sum(case[key] for case in cases)}/{len(cases)}")
print(f"Largest position error: {max(case['max_position_error_points'] for case in cases)} points")
print(f"Saved {args.output}")
if args.check_invariants and not result["representation_invariants_pass"]:
    raise SystemExit("Native representation invariants failed; inspect report")
