"""Independent geometric/structural review of semantic Type3 probe artifacts.

Consumes existing raw PDFium inspection plus freshly reads PDF structure and
MuPDF raw character boxes. No runtime code or prototype PDF is modified.
"""
import argparse
import json
from pathlib import Path

import fitz
from pypdf import PdfReader
from pypdf.generic import ContentStream

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--input", type=Path, default=ROOT / "artifacts/shaped-text/semantic-rotations")
parser.add_argument("--native", type=Path, default=ROOT / "artifacts/shaped-text/native")
parser.add_argument("--output", type=Path, default=ROOT / "artifacts/shaped-text/semantic-rotations/geometry-review.json")
args = parser.parse_args()
records = json.loads((args.input / "inputs.json").read_text(encoding="utf-8"))
inspection = json.loads((args.input / "inspection/results.json").read_text(encoding="utf-8"))
reader = PdfReader(args.input / "semantic.pdf")
saved_reader = PdfReader(args.input / "inspection/pdfium-resaved.pdf")


def structure(page, pdf):
    fonts = page["/Resources"]["/Font"]
    programs = []
    for font in fonts.values():
        font = font.get_object()
        assert font["/Subtype"] == "/Type3"
        for name, ref in font["/CharProcs"].items():
            ops = ContentStream(ref.get_object(), pdf).operations
            programs.append(dict(name=name, blank_d1_only=len(ops) == 1 and ops[0][1] == b"d1" and len(ops[0][0]) == 6))
    ops = ContentStream(page.get_contents(), pdf).operations
    operators = [op for _, op in ops]
    return dict(font_count=len(fonts), charproc_count=len(programs),
                all_programs_blank_d1=bool(programs) and all(p["blank_d1_only"] for p in programs),
                text_objects=operators.count(b"BT"), text_draws=operators.count(b"TJ") + operators.count(b"Tj"),
                vector_fills=operators.count(b"f"), actualtext_spans=operators.count(b"BDC"))


def compare(actual, expected):
    return dict(actual=actual, expected=expected,
                max_edge_error=max(abs(a - b) for a, b in zip(actual, expected)),
                maximum_ink_cut=max(0, actual[0] - expected[0], expected[1] - actual[1], actual[2] - expected[2], expected[3] - actual[3]))


results = []
with fitz.open(args.input / "semantic.pdf") as doc, fitz.open(args.input / "inspection/pdfium-resaved.pdf") as saved:
    for i, record in enumerate(records):
        if record["order"] != "visual-tj":
            continue
        layout = json.loads((args.native / f"{record['case']}-0.layout.json").read_text(encoding="utf-8"))
        bounds = layout["bounds"]
        origin = [48 - min(0, bounds["xMin"]), 48 - min(0, bounds["yMin"])]
        native_clusters = {}
        for run in layout["runs"]:
            for glyph in run["glyphs"]:
                key = (glyph["cluster"]["utf8Start"], glyph["cluster"]["utf8End"])
                native_clusters.setdefault(key, [])
                if glyph["bounds"]:
                    native_clusters[key].append(glyph["bounds"])
        expected = []
        source_offset = 0
        for cell in record["cells"]:
            box = [origin[0] + cell["bxmin"], origin[0] + cell["bxmax"], origin[1] + cell["ymin"], origin[1] + cell["ymax"]]
            cluster_ink = next(ink for (start, end), ink in native_clusters.items() if start <= source_offset < end)
            ink_box = [origin[0] + min(b["xMin"] for b in cluster_ink), origin[0] + max(b["xMax"] for b in cluster_ink),
                       origin[1] + min(b["yMin"] for b in cluster_ink), origin[1] + max(b["yMax"] for b in cluster_ink)] if cluster_ink else None
            encoded = cell["char"].encode("utf-16-le")
            for j in range(0, len(encoded), 2):
                expected.append(dict(unicode=int.from_bytes(encoded[j:j + 2], "little"), box=box, native_ink=ink_box))
            source_offset += len(cell["char"].encode("utf-8"))
        actual = inspection["pdfium"]["original"][i]["characters"]
        comparable = len(actual) == len(expected) and [x["unicode"] for x in actual] == [x["unicode"] for x in expected]
        comparisons = [compare(a["box"], e["box"]) for a, e in zip(actual, expected)] if comparable else []
        ink_comparisons = [compare(a["box"], e["native_ink"]) for a, e in zip(actual, expected) if e["native_ink"]] if comparable else []
        def raw_chars(page, flags=None):
            raw = page.get_text("rawdict", flags=flags) if flags is not None else page.get_text("rawdict")
            return [char for block in raw["blocks"] if "lines" in block
                    for line in block["lines"] for span in line["spans"] for char in span["chars"]]
        chars = raw_chars(doc[i])
        mu_comparable = "".join(c["c"] for c in chars) == record["text"]
        mu_comparisons = []
        if mu_comparable:
            height = float(reader.pages[i].mediabox.height)
            for char, cell in zip(chars, record["cells"]):
                left, top, right, bottom = char["bbox"]
                expected_box = [origin[0] + cell["bxmin"], origin[0] + cell["bxmax"], origin[1] + cell["ymin"], origin[1] + cell["ymax"]]
                mu_comparisons.append(compare([left, right, height - bottom, height - top], expected_box))
        before = structure(reader.pages[i], reader)
        after = structure(saved_reader.pages[i], saved_reader)
        results.append(dict(case=record["case"], rotation=record["rotation"], page=i,
                            text=record["text"], source_struct=before, saved_struct=after,
                            exact_all_readers=all(text.rstrip("\r\n") == record["text"] for text in [inspection["pdfium"]["original"][i]["text"], inspection["original"]["mupdf"][i], inspection["original"]["pypdf"][i]]),
                            pdfium_unicode_units_match=comparable, pdfium_boxes=comparisons,
                            semantic_cells_match_logical_text="".join(cell["char"] for cell in record["cells"]) == layout["text"],
                            pdfium_native_ink=ink_comparisons,
                            pdfium_boxes_unchanged=actual == inspection["pdfium"]["resaved"][i]["characters"],
                            mupdf_characters=chars, mupdf_cluster_boxes=mu_comparisons,
                            mupdf_accurate_characters=raw_chars(doc[i], fitz.TEXTFLAGS_RAWDICT | fitz.TEXT_ACCURATE_BBOXES),
                            mupdf_accurate_side_bearings_characters=raw_chars(doc[i], fitz.TEXTFLAGS_RAWDICT | fitz.TEXT_ACCURATE_BBOXES | fitz.TEXT_ACCURATE_SIDE_BEARINGS),
                            mupdf_boxes_unchanged=chars == raw_chars(saved[i]),
                            mupdf_raster_unchanged=inspection["original"]["raster_sha256"][i] == inspection["resaved"]["raster_sha256"][i]))
positive = [row for row in results if row["exact_all_readers"]]
report = dict(cases=results, positive_cases=len(positive),
              pdfium_max_positive_box_error=max(b["max_edge_error"] for row in positive for b in row["pdfium_boxes"]),
              pdfium_max_positive_native_ink_cut=max(b["maximum_ink_cut"] for row in positive for b in row["pdfium_native_ink"]),
              mupdf_max_positive_box_error=max(b["max_edge_error"] for row in positive for b in row["mupdf_cluster_boxes"]),
              limitation="PDFium raw boxes were measured by the existing inspector; this script rechecks PDF structure and MuPDF directly. Expected boxes include cluster advance plus native ink, not only tight ink. Type3 blank programs draw nothing; their advertised d1 bounds are semantic metadata.")
args.output.write_text(json.dumps(report, ensure_ascii=True, indent=2), encoding="utf-8")
print(json.dumps({key: value for key, value in report.items() if key not in ("cases", "limitation")}))
print(f"Reviewed {len(results)} visual-TJ pages; report {args.output}")
