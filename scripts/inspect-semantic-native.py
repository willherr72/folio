"""Independent inspection of actual native semantic-text evidence.

Checks raw MuPDF/pypdf text, original font bytes, blank Type3 CharProcs, one
semantic text object with banked TJ arrays, vectors, save preservation, and engine display-coordinate
cluster boxes against native layout. Does not generate or modify source PDFs.
"""
import argparse
import hashlib
import importlib.metadata
import json
import math
from pathlib import Path
import re

import fitz
from pypdf import PdfReader
from pypdf.generic import ArrayObject, ContentStream, ByteStringObject, TextStringObject

ROOT = Path(__file__).resolve().parents[1]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def semantic_text(fonts, operations, reader, expected_visual=None):
    """Track the selected bank for every byte within one native text object."""
    result = dict(passes=False, scalar_use_count=0, decoded_visual_text="")
    checks = []
    try:
        grouped = {name: [] for name in fonts}
        order = []
        active, begun, ended, positioned = False, False, False, False
        current, size, pending_array = None, None, False
        for values, operator in operations:
            if operator == b"BT":
                if values or begun:
                    raise ValueError("Expected exactly one nonnested BT/ET text object")
                active = begun = True
            elif operator == b"ET":
                if values or not active or pending_array:
                    raise ValueError("Unmatched or malformed ET")
                active, ended = False, True
            elif operator == b"Tf":
                if (not active or len(values) != 2 or str(values[0]) not in fonts
                        or not isinstance(values[1], (int, float))
                        or not math.isfinite(float(values[1])) or float(values[1]) <= 0):
                    raise ValueError("Invalid semantic font selection")
                if size is not None and float(values[1]) != size:
                    raise ValueError("Semantic bank switch changed font size")
                if pending_array or str(values[0]) == current:
                    raise ValueError("Semantic font selection must introduce a bank's TJ array")
                current, size = str(values[0]), float(values[1])
                pending_array = True
            elif operator == b"Tm":
                if (not active or positioned or len(values) != 6
                        or not all(isinstance(v, (int, float)) and math.isfinite(float(v)) for v in values)
                        or list(map(float, values[:4])) != [1, 0, 0, 1]):
                    raise ValueError("Invalid or repeated native text matrix")
                positioned = True
            elif operator == b"TJ":
                if not active or current is None or not positioned or not pending_array or len(values) != 1 or not isinstance(values[0], ArrayObject):
                    raise ValueError("Malformed or misplaced TJ array")
                pending_array = False
                grouped[current].append((values, operator))
                for item in values[0]:
                    if isinstance(item, ByteStringObject):
                        order.extend([current] * len(bytes(item)))
                    elif isinstance(item, TextStringObject):
                        order.extend([current] * len(item.original_bytes))
                    elif not isinstance(item, (int, float)) or not math.isfinite(float(item)):
                        raise ValueError("Invalid TJ element")
            elif active or operator not in (b"q", b"Q", b"cm", b"rg", b"g", b"m", b"l", b"c", b"h", b"re", b"f"):
                raise ValueError("Unsupported native content operator")
        if not begun or not ended or active or not order:
            raise ValueError("Incomplete or empty semantic text object")
        checks = [dict(resource=name, **semantic_codes(font, grouped[name], reader)) for name, font in fonts.items()]
        if not all(check["passes"] for check in checks):
            raise ValueError("Semantic bank definitions or emitted codes failed validation")
        decoded_banks = {check["resource"]: iter(check["decoded_visual_text"]) for check in checks}
        decoded = "".join(next(decoded_banks[name]) for name in order)
        result.update(scalar_use_count=len(order), decoded_visual_text=decoded,
                      source_visual_matches=expected_visual is None or decoded == expected_visual)
        if not result["source_visual_matches"]:
            raise ValueError("CMap text disagrees with native visual scalar order")
        result["passes"] = True
    except (KeyError, ValueError, TypeError, IndexError, UnicodeError, StopIteration) as error:
        result["error"] = str(error)
    return checks, result


def semantic_codes(font, operations, reader, expected_visual=None):
    """Inspect the native single-byte/bfchar representation, failing closed."""
    result = dict(passes=False, scalar_use_count=0, unique_code_count=0)
    try:
        first, last = int(font["/FirstChar"]), int(font["/LastChar"])
        if not 1 <= first <= last <= 255:
            raise ValueError("Semantic code range must fit nonzero single bytes")
        codes = set(range(first, last + 1))
        widths = font["/Widths"]
        if len(widths) != len(codes):
            raise ValueError("Widths do not cover code range")
        encoding, code = {}, None
        for item in font["/Encoding"]["/Differences"]:
            if isinstance(item, int):
                code = int(item)
            else:
                if code is None or code in encoding or not str(item).startswith("/"):
                    raise ValueError("Malformed or duplicate encoding code")
                encoding[code] = str(item)
                code += 1
        procs = font["/CharProcs"]
        if set(encoding) != codes or len(set(encoding.values())) != len(codes) or set(encoding.values()) != set(procs):
            raise ValueError("Encoding/CharProcs coverage or alias mismatch")
        for code, name in encoding.items():
            ops = ContentStream(procs[name].get_object(), reader).operations
            if len(ops) != 1 or ops[0][1] != b"d1" or len(ops[0][0]) != 6:
                raise ValueError("Character program must contain only d1")
            values = list(map(float, ops[0][0]))
            if (not all(math.isfinite(value) for value in values) or values[1] != 0
                    or values[0] != float(widths[code - first])
                    or values[2] > values[4] or values[3] > values[5]):
                raise ValueError("Character program width or bounding box mismatch")
        cmap = re.sub(r"%[^\r\n]*", "", font["/ToUnicode"].get_data().decode("ascii"))
        spaces = re.findall(r"(\d+)\s+begincodespacerange\s*(.*?)\s*endcodespacerange", cmap, re.S)
        if len(spaces) != 1 or spaces[0][0] != "1" or re.sub(r"\s+", "", spaces[0][1]).upper() != "<00><FF>":
            raise ValueError("CMap must declare the native single-byte code space")
        if re.search(r"\b(?:beginbfrange|begincidchar|begincidrange|usecmap)\b", cmap):
            raise ValueError("Unsupported native CMap mapping operator")
        mappings = {}
        blocks = re.findall(r"(\d+)\s+beginbfchar\s*(.*?)\s*endbfchar", cmap, re.S)
        if len(blocks) != len(re.findall(r"\bbeginbfchar\b", cmap)):
            raise ValueError("Malformed CMap mapping block")
        pair = r"<([0-9A-Fa-f]{2})>\s*<([0-9A-Fa-f]+)>"
        for count, body in blocks:
            entries = re.findall(pair, body)
            if len(entries) != int(count) or re.sub(pair, "", body).strip():
                raise ValueError("Malformed CMap definition or count")
            for raw_code, raw_unicode in entries:
                code = int(raw_code, 16)
                unicode = bytes.fromhex(raw_unicode).decode("utf-16-be")
                if code in mappings or len(unicode) != 1:
                    raise ValueError("Duplicate CMap code or non-scalar mapping")
                mappings[code] = unicode
        if set(mappings) != codes:
            raise ValueError("CMap does not cover exactly the defined codes")
        uses = []
        for values, operator in operations:
            if operator == b"TJ":
                for item in values[0]:
                    if isinstance(item, ByteStringObject):
                        uses.extend(bytes(item))
                    elif isinstance(item, TextStringObject):
                        uses.extend(item.original_bytes)
                    elif not isinstance(item, (int, float)) or not math.isfinite(float(item)):
                        raise ValueError("Invalid TJ element")
        if not uses or set(uses) != codes:
            raise ValueError("Emitted codes and font definitions differ")
        decoded = "".join(mappings[code] for code in uses)
        result.update(scalar_use_count=len(uses), unique_code_count=len(codes), decoded_visual_text=decoded,
                      source_visual_matches=expected_visual is None or decoded == expected_visual)
        if not result["source_visual_matches"]:
            raise ValueError("CMap text disagrees with native visual scalar order")
        result["passes"] = True
    except (KeyError, ValueError, TypeError, IndexError, UnicodeError) as error:
        result["error"] = str(error)
    return result


def inspect_pdf(path, expected_font, expected_visual=None):
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
    code_checks, text_check = semantic_text({name: fonts[name].get_object() for name in type3}, operations, reader, expected_visual)
    used_fonts = [str(values[0]) for values, operator in operations if operator == b"Tf" and values]
    structure = dict(type3_fonts=type3, retained_original_fonts=original,
                     charproc_count=len(programs), all_charprocs_blank_d1=bool(programs) and all(p["blank_d1_only"] for p in programs),
                     text_objects=operators.count(b"BT"), tj_arrays=operators.count(b"TJ"),
                     other_text_draws=sum(operators.count(op) for op in (b"Tj", b"'", b'"')),
                     actualtext_spans=operators.count(b"BDC"), vector_fills=operators.count(b"f"),
                     used_fonts=used_fonts,
                     semantic_codes=code_checks,
                     semantic_text=text_check,
                     original_font_matches=bool(original) and all(f["sha256"] == expected_font for f in original),
                     original_font_unused=bool(original) and all(f["resource"] not in used_fonts for f in original))
    structure["passes"] = (len(type3) >= 1 and len(original) == 1 and structure["all_charprocs_blank_d1"]
                           and structure["text_objects"] == 1 and structure["tj_arrays"] >= 1
                           and structure["other_text_draws"] == structure["actualtext_spans"] == 0
                           and structure["vector_fills"] > 0 and set(used_fonts) == set(type3)
                           and text_check["passes"]
                           and all(check["passes"] for check in code_checks)
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


def visual_text(layout):
    """Read source cluster scalars in increasing native horizontal position."""
    clusters = {}
    for run in layout["runs"]:
        for glyph in run["glyphs"]:
            span = glyph["cluster"]
            clusters.setdefault((span["utf8Start"], span["utf8End"]), (run["direction"], []))[1].append(glyph)
    cells = []
    for (start, end), (direction, glyphs) in sorted(clusters.items()):
        text = layout["text"].encode("utf-8")[start:end].decode("utf-8")
        left = min(g["x"] - g["xOffset"] for g in glyphs)
        advance = sum(g["xAdvance"] for g in glyphs) / len(text)
        for i, char in enumerate(text):
            index = len(text) - i - 1 if direction == "rtl" else i
            cells.append((left + advance * index, char))
    return "".join(char for _, char in sorted(cells, key=lambda cell: cell[0]))


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
    parser.add_argument("--long-text", action="store_true", help="Require the 28-case long-text matrix; use separate output and summary paths")
    parser.add_argument("--geometry-tolerance", type=float, default=.06, help="Point tolerance for these fixtures; not a universal production promise")
    args = parser.parse_args()
    if args.long_text and args.summary.resolve() == (ROOT / "docs/semantic-native-verification.json").resolve():
        parser.error("--long-text requires a separate --summary path to preserve the historical short-text report")
    if args.long_text and args.output.resolve() == (ROOT / "artifacts/shaped-text/semantic-native-independent.json").resolve():
        parser.error("--long-text requires a separate --output path to preserve the historical short-text report")
    evidence = json.loads((args.input / "results.json").read_text(encoding="utf-8"))
    rows = []
    for entry in evidence["cases"]:
        original_path = args.input / entry["pdf"]
        layout = json.loads(original_path.with_suffix(".layout.json").read_text(encoding="utf-8"))
        expected = expected_clusters(layout)
        visual = visual_text(layout)
        original = inspect_pdf(original_path, layout["fontId"], visual)
        resaved = inspect_pdf(args.input / entry["resavedPdf"], layout["fontId"], visual)
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
                   semantic_scalar_count_matches=all(pdf["structure"]["semantic_text"]["scalar_use_count"] == len(layout["text"]) for pdf in (original, resaved)),
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
    mixed = [r for r in refused if r["case"] == "mixed-bidi"]
    rtl_words = [r for r in refused if r["case"] == "rtl-word-boundaries"]
    refusal_gate = len(mixed) == 4 and {r["rotation"] for r in mixed} == {0, 90, 180, 270}
    refusal_gate = refusal_gate and all(r.get("source") == "ABC \u0633\u0644\u0627\u0645 123 DEF" and r.get("reason") for r in mixed)
    rtl_refusal_gate = (len(rtl_words) == 4 and {r["rotation"] for r in rtl_words} == {0, 90, 180, 270}) if args.long_text else not rtl_words
    rtl_refusal_gate = rtl_refusal_gate and all(r.get("source") == " ".join(["\u0633\u0644\u0627\u0645 \u0639\u0627\u0644\u0645"] * 32) and r.get("reason") for r in rtl_words)
    refusals_complete = len(refused) == (8 if args.long_text else 4) and refusal_gate and rtl_refusal_gate
    case_names = (("long-ligatures-on", "long-ligatures-off", "long-marks", "long-arabic", "long-indic", "long-supplementary", "maximum-scalars")
                  if args.long_text else ("ligatures-on", "ligatures-off", "composed", "decomposed", "two-axis-marks", "arabic", "indic", "supplementary"))
    expected_cases = {(name, rotation) for name in case_names for rotation in (0, 90, 180, 270)}
    matrix_complete = len(rows) == len(expected_cases) and {(row["case"], row["rotation"]) for row in rows} == expected_cases
    if args.long_text:
        long_sources = {"long-ligatures-on": " ".join(["office"] * 64),
                        "long-ligatures-off": " ".join(["office"] * 64),
                        "long-marks": " ".join(["q\u0307\u0323"] * 80),
                        "long-arabic": "\u0633\u0644\u0627\u0645" * 80,
                        "long-indic": " ".join(["\u0915\u093f\u0924\u093e\u092c"] * 64),
                        "long-supplementary": "A\U0001d434B" * 128,
                        "maximum-scalars": "i" * 4096}
        matrix_complete = matrix_complete and all(row["source"] == long_sources.get(row["case"]) for row in rows)
    report = dict(versions={name: importlib.metadata.version(name) for name in ("pymupdf", "pypdf")},
                  pdfium_sha256=evidence["pdfiumSha256"], representation=evidence["representation"],
                  geometry_tolerance_points=args.geometry_tolerance, cases=rows, refused=refused,
                  matrix="long-text" if args.long_text else "short-text",
                  matrix_complete=matrix_complete, mixed_refusal_gate=refusal_gate, rtl_boundary_refusal_gate=rtl_refusal_gate,
                  invariants_pass=matrix_complete and refusals_complete and all(r["invariants_pass"] for r in rows),
                  limitation="PDFium text/geometry/raster hashes are engine-produced evidence; MuPDF/pypdf and PDF structure independently read here. MuPDF horizontal cluster geometry is recorded separately and is not claimed equivalent to PDFium.")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=True, indent=2), encoding="utf-8")
    summary = {key: value for key, value in report.items() if key != "cases"}
    summary["scope"] = "Native fixture verification only; per-case results record whether PDFium cluster geometry meets measured tolerance. MuPDF horizontal cluster geometry remains different. No production or complete-script-support claim."
    summary["raw_comparison"] = "Only final CR/LF reader terminators ignored for exactness; raw original/exported strings below are unmodified."
    summary["cases"] = [dict(case=row["case"], rotation=row["rotation"], source=row["source"],
                             raw=row["raw"], raw_resaved=row["raw_resaved"],
                             original_pdf_sha256=row["original"]["sha256"], resaved_pdf_sha256=row["resaved"]["sha256"],
                             retained_font_sha256=next((f["sha256"] for f in row["original"]["structure"]["retained_original_fonts"]), None),
                             source_scalar_count=len(row["source"]),
                             original_semantic_codes=row["original"]["structure"]["semantic_codes"],
                             resaved_semantic_codes=row["resaved"]["structure"]["semantic_codes"],
                             mupdf_original_raster_sha256=row["original"]["raster_sha256"], mupdf_resaved_raster_sha256=row["resaved"]["raster_sha256"],
                             pdfium_max_cluster_edge_error_points=max((g["max_ink_cluster_edge_error"] for g in row["engine_geometry"] if g["max_ink_cluster_edge_error"] is not None), default=None),
                             mupdf_max_cluster_edge_error_points=max((g["edge_error_points"] for g in row["mupdf_geometry"] if g["ink_present"]), default=None),
                             exact_all_readers=row["exact_all_readers"], invariants_pass=row["invariants_pass"]) for row in rows]
    args.summary.parent.mkdir(parents=True, exist_ok=True)
    args.summary.write_text(json.dumps(summary, ensure_ascii=True, indent=2) + "\n", encoding="utf-8")
    for key in ("exact_all_readers", "engine_geometry_within_tolerance", "engine_geometry_unchanged", "mupdf_raster_unchanged", "pdfium_raster_unchanged", "invariants_pass"):
        print(f"{key}: {sum(r[key] for r in rows)}/{len(rows)}")
    print(f"Mixed refusal gate: {refusal_gate}; RTL boundary refusal gate: {rtl_refusal_gate}; report {args.output}")
    if args.check_invariants and not report["invariants_pass"]:
        raise SystemExit("Native semantic invariants failed; inspect raw report")


if __name__ == "__main__":
    main()
