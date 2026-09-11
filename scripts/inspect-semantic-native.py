"""Independent inspection of actual native semantic-text evidence.

Checks raw MuPDF/pypdf text, original font bytes, blank Type3 CharProcs, one
semantic TJ object, vectors, save preservation, and engine display-coordinate
cluster boxes against native layout. Does not generate or modify source PDFs.
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


def digest(data):
    return hashlib.sha256(data).hexdigest()


def inspect_pdf(path, expected_font):
    reader = PdfReader(path)
    if len(reader.pages) != 1:
        raise ValueError(f"Expected one native evidence page: {path}")
    page = reader.pages[0]
    fonts = page["/Resources"]["/Font"]
    type3, original, programs = [], [], []
    for name, ref in fonts.items():
        font = ref.get_object()
        if font["/Subtype"] == "/Type3":
            type3.append(name)
            for proc_name, proc in font["/CharProcs"].items():
                operations = ContentStream(proc.get_object(), reader).operations
                valid = len(operations) == 1 and operations[0][1] == b"d1" and len(operations[0][0]) == 6
                programs.append(dict(name=proc_name, blank_d1_only=valid))
        elif font["/Subtype"] == "/Type0":
            data = font["/DescendantFonts"][0].get_object()["/FontDescriptor"]["/FontFile2"].get_data()
            original.append(dict(resource=name, bytes=len(data), sha256=digest(data)))
    operations = ContentStream(page.get_contents(), reader).operations
    operators = [operator for _, operator in operations]
    used_fonts = [str(values[0]) for values, operator in operations if operator == b"Tf"]
    structure = dict(type3_fonts=type3, retained_original_fonts=original,
                     charproc_count=len(programs), all_charprocs_blank_d1=bool(programs) and all(p["blank_d1_only"] for p in programs),
                     text_objects=operators.count(b"BT"), tj_arrays=operators.count(b"TJ"),
                     other_text_draws=sum(operators.count(op) for op in (b"Tj", b"'", b'"')),
                     actualtext_spans=operators.count(b"BDC"), vector_fills=operators.count(b"f"),
                     used_fonts=used_fonts,
                     original_font_matches=bool(original) and all(f["sha256"] == expected_font for f in original),
                     original_font_unused=bool(original) and all(f["resource"] not in used_fonts for f in original))
    structure["passes"] = (len(type3) == 1 and len(original) == 1 and structure["all_charprocs_blank_d1"]
                           and structure["text_objects"] == structure["tj_arrays"] == 1
                           and structure["other_text_draws"] == structure["actualtext_spans"] == 0
                           and structure["vector_fills"] > 0 and used_fonts == type3
                           and structure["original_font_matches"] and structure["original_font_unused"])
    with fitz.open(path) as mupdf:
        rendered = mupdf[0].get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False)
        raw = mupdf[0].get_text("rawdict")
        chars = [char for block in raw["blocks"] if "lines" in block
                 for line in block["lines"] for span in line["spans"] for char in span["chars"]]
        return dict(path=str(path.resolve()), sha256=digest(path.read_bytes()), structure=structure,
                    media_box=list(map(float, page.mediabox)), rotation=int(page.get("/Rotate", 0)),
                    mupdf=mupdf[0].get_text(), pypdf=page.extract_text(), mupdf_characters=chars,
                    raster_sha256=digest(rendered.samples), raster_size=[rendered.width, rendered.height])


def expected_clusters(layout):
    bounds = layout["bounds"]
    origin = [48 - min(0, bounds["xMin"]), 48 - min(0, bounds["yMin"])]
    clusters = {}
    for run in layout["runs"]:
        for glyph in run["glyphs"]:
            span = glyph["cluster"]
            clusters.setdefault((span["utf8Start"], span["utf8End"]), []).append(glyph)
    result = []
    for (start, end), glyphs in sorted(clusters.items()):
        text = layout["text"].encode("utf-8")[start:end].decode("utf-8")
        ink = [g["bounds"] for g in glyphs if g["bounds"]]
        left = min(g["x"] - g["xOffset"] for g in glyphs)
        right = left + sum(g["xAdvance"] for g in glyphs)
        if ink:
            raw_ink = [min(b["xMin"] for b in ink), max(b["xMax"] for b in ink),
                       min(b["yMin"] for b in ink), max(b["yMax"] for b in ink)]
            tight = [raw_ink[0] + origin[0], raw_ink[1] + origin[0], raw_ink[2] + origin[1], raw_ink[3] + origin[1]]
            union = [min(left, raw_ink[0]) + origin[0], max(right, raw_ink[1]) + origin[0], tight[2], tight[3]]
        else:
            tight = None
            union = [left + origin[0], right + origin[0], origin[1] - .2 * layout["fontSize"], origin[1] + .8 * layout["fontSize"]]
        result.extend(dict(text=char, range_utf8=[start, end], cluster_box=union, ink_box=tight) for char in text)
    if "".join(c["text"] for c in result) != layout["text"]:
        raise ValueError("Native clusters do not cover logical source exactly once")
    return result


def displayed_box(box, media, rotation):
    left, bottom, right, top = media
    def point(x, y):
        return {0: (x - left, top - y), 90: (y - bottom, x - left),
                180: (right - x, y - bottom), 270: (top - y, right - x)}[rotation]
    a, b = point(box[0], box[2]), point(box[1], box[3])
    return [min(a[0], b[0]), max(a[0], b[0]), min(a[1], b[1]), max(a[1], b[1])]


def box_error(actual, expected):
    return max(abs(a - b) for a, b in zip(actual, expected))


def engine_geometry(geometry, expected, pdf):
    chars = geometry["characters"]
    source_matches = len(chars) == len(expected) and all(a["text"] == e["text"] for a, e in zip(chars, expected))
    comparisons = []
    if source_matches:
        for actual, desired in zip(chars, expected):
            target = displayed_box(desired["cluster_box"], pdf["media_box"], geometry["intrinsicRotation"])
            box = [actual["x"], actual["x"] + actual["width"], actual["y"], actual["y"] + actual["height"]]
            comparisons.append(dict(text=desired["text"], cluster=desired["range_utf8"],
                                    actual=box, expected=target, ink_present=desired["ink_box"] is not None,
                                    edge_error_points=box_error(box, target)))
    # Engine deliberately prefers loose font bounds for whitespace. Whitespace
    # is recorded but cannot establish or invalidate tight visible-cluster ink.
    ink = [c["edge_error_points"] for c in comparisons if c["ink_present"]]
    return dict(source_scalars_match=source_matches, comparisons=comparisons,
                max_ink_cluster_edge_error=max(ink) if ink else None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, default=ROOT / "artifacts/shaped-text/semantic-native")
    parser.add_argument("--output", type=Path, default=ROOT / "artifacts/shaped-text/semantic-native-independent.json")
    parser.add_argument("--summary", type=Path, default=ROOT / "docs/semantic-native-verification.json")
    parser.add_argument("--check-invariants", action="store_true")
    parser.add_argument("--geometry-tolerance", type=float, default=.06, help="Point tolerance for these 24pt fixtures; not a universal production promise")
    args = parser.parse_args()
    evidence = json.loads((args.input / "results.json").read_text(encoding="utf-8"))
    rows = []
    for entry in evidence["cases"]:
        original_path = args.input / entry["pdf"]
        layout = json.loads(original_path.with_suffix(".layout.json").read_text(encoding="utf-8"))
        expected = expected_clusters(layout)
        original = inspect_pdf(original_path, layout["fontId"])
        resaved = inspect_pdf(args.input / entry["resavedPdf"], layout["fontId"])
        geometries = [engine_geometry(entry[key], expected, pdf) for key, pdf in
                      (("originalGeometry", original), ("resavedGeometry", resaved))]
        mu_matches = "".join(c["c"] for c in original["mupdf_characters"]) == layout["text"]
        mu_geometry = []
        if mu_matches:
            height = original["media_box"][3]
            for actual, desired in zip(original["mupdf_characters"], expected):
                left, top, right, bottom = actual["bbox"]
                box = [left, right, height - bottom, height - top]
                mu_geometry.append(dict(text=desired["text"], actual=box, expected=desired["cluster_box"],
                                        ink_present=desired["ink_box"] is not None,
                                        edge_error_points=box_error(box, desired["cluster_box"])))
        raw = dict(pdfium=entry["originalText"], mupdf=original["mupdf"], pypdf=original["pypdf"])
        raw_after = dict(pdfium=entry["resavedText"], mupdf=resaved["mupdf"], pypdf=resaved["pypdf"])
        exact = all(value.rstrip("\r\n") == layout["text"] for values in (raw, raw_after) for value in values.values())
        geometry_pass = all(g["source_scalars_match"] and g["max_ink_cluster_edge_error"] is not None and
                            g["max_ink_cluster_edge_error"] <= args.geometry_tolerance for g in geometries)
        row = dict(case=entry["case"], rotation=entry["rotation"], source=layout["text"],
                   metadata_matches=entry["fontSha256"] == layout["fontId"] and
                       all(pdf["rotation"] == entry["rotation"] for pdf in (original, resaved)) and
                       all(entry[key]["intrinsicRotation"] == entry["rotation"] for key in ("originalGeometry", "resavedGeometry")),
                   semantic_scalar_count_matches=all(pdf["structure"]["charproc_count"] == len(layout["text"]) for pdf in (original, resaved)),
                   original=original, resaved=resaved, raw=raw, raw_resaved=raw_after, exact_all_readers=exact,
                   engine_geometry=geometries, engine_geometry_within_tolerance=geometry_pass,
                   engine_geometry_unchanged=entry["originalGeometry"] == entry["resavedGeometry"],
                   mupdf_geometry=mu_geometry, mupdf_geometry_unchanged=original["mupdf_characters"] == resaved["mupdf_characters"],
                   mupdf_raster_unchanged=original["raster_sha256"] == resaved["raster_sha256"],
                   pdfium_raster_unchanged=entry["originalPngSha256"] == entry["resavedPngSha256"],
                   raw_unchanged=raw == raw_after)
        row["invariants_pass"] = (exact and geometry_pass and original["structure"]["passes"] and resaved["structure"]["passes"] and
                                  all(row[k] for k in ("metadata_matches", "semantic_scalar_count_matches", "engine_geometry_unchanged", "mupdf_geometry_unchanged", "mupdf_raster_unchanged", "pdfium_raster_unchanged", "raw_unchanged")))
        rows.append(row)
    refused = evidence.get("refused", [])
    refusal_gate = len(refused) == 4 and {r["rotation"] for r in refused} == {0, 90, 180, 270} and all(r["case"] == "mixed-bidi" for r in refused)
    expected_cases = {(name, rotation) for name in ("ligatures-on", "ligatures-off", "composed", "decomposed", "two-axis-marks", "arabic", "indic", "supplementary") for rotation in (0, 90, 180, 270)}
    matrix_complete = len(rows) == 32 and {(row["case"], row["rotation"]) for row in rows} == expected_cases
    report = dict(versions={name: importlib.metadata.version(name) for name in ("pymupdf", "pypdf")},
                  pdfium_sha256=evidence["pdfiumSha256"], representation=evidence["representation"],
                  geometry_tolerance_points=args.geometry_tolerance, cases=rows, refused=refused,
                  matrix_complete=matrix_complete, mixed_refusal_gate=refusal_gate, invariants_pass=matrix_complete and refusal_gate and all(r["invariants_pass"] for r in rows),
                  limitation="PDFium text/geometry/raster hashes are engine-produced evidence; MuPDF/pypdf and PDF structure independently read here. MuPDF horizontal cluster geometry is recorded separately and is not claimed equivalent to PDFium.")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=True, indent=2), encoding="utf-8")
    summary = {key: value for key, value in report.items() if key != "cases"}
    summary["scope"] = "Native fixture verification only; PDFium cluster geometry passes measured tolerance, MuPDF horizontal cluster geometry remains different. No production or complete-script-support claim."
    summary["raw_comparison"] = "Only final CR/LF reader terminators ignored for exactness; raw original/exported strings below are unmodified."
    summary["cases"] = [dict(case=row["case"], rotation=row["rotation"], source=row["source"],
                             raw=row["raw"], raw_resaved=row["raw_resaved"],
                             original_pdf_sha256=row["original"]["sha256"], resaved_pdf_sha256=row["resaved"]["sha256"],
                             retained_font_sha256=next((f["sha256"] for f in row["original"]["structure"]["retained_original_fonts"]), None),
                             mupdf_original_raster_sha256=row["original"]["raster_sha256"], mupdf_resaved_raster_sha256=row["resaved"]["raster_sha256"],
                             pdfium_max_cluster_edge_error_points=max((g["max_ink_cluster_edge_error"] for g in row["engine_geometry"] if g["max_ink_cluster_edge_error"] is not None), default=None),
                             mupdf_max_cluster_edge_error_points=max((g["edge_error_points"] for g in row["mupdf_geometry"] if g["ink_present"]), default=None),
                             exact_all_readers=row["exact_all_readers"], invariants_pass=row["invariants_pass"]) for row in rows]
    args.summary.parent.mkdir(parents=True, exist_ok=True)
    args.summary.write_text(json.dumps(summary, ensure_ascii=True, indent=2) + "\n", encoding="utf-8")
    for key in ("exact_all_readers", "engine_geometry_within_tolerance", "engine_geometry_unchanged", "mupdf_raster_unchanged", "pdfium_raster_unchanged", "invariants_pass"):
        print(f"{key}: {sum(r[key] for r in rows)}/{len(rows)}")
    print(f"Mixed refusal gate: {refusal_gate}; report {args.output}")
    if args.check_invariants and not report["invariants_pass"]:
        raise SystemExit("Native semantic invariants failed; inspect raw report")


if __name__ == "__main__":
    main()
