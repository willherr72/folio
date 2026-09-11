"""Bounded #14 experiment: grouped TJ, logical cluster maps, ActualText scope.

Uses existing native layout/PDF evidence, never reshapes text. No extracted
Unicode is reversed or normalized. Only final reader CR/LF terminators are
ignored for exactness; raw strings and boxes remain in the JSON report.
"""
import argparse
import ctypes as c
import hashlib
import importlib.metadata
import json
from pathlib import Path

import fitz
from pypdf import PdfReader, PdfWriter
from pypdf.generic import ArrayObject, ContentStream, DecodedStreamObject, NameObject, NumberObject

ROOT = Path(__file__).resolve().parents[1]
MODES = ("whole_grouped", "run_actual_visual", "run_actual_logical",
         "cluster_actual_logical", "cluster_unicode_logical", "cluster_unicode_visual",
         "run_unicode_visual")


def sha(data):
    return hashlib.sha256(data).hexdigest()


def stream(writer, data):
    obj = DecodedStreamObject()
    obj.set_data(data)
    return writer._add_object(obj)


def actual(text):
    encoded = ("\ufeff" + text).encode("utf-16-be").hex()
    return f"/Span << /ActualText <{encoded}> >> BDC"


def grouped_tj(glyphs, size, font_name):
    """A TJ cannot express vertical offsets: split when actual y changes.

    All x positions are expressed through inter-glyph TJ adjustments. Keeping
    an explicit split for y preserves the native ink; do not silently flatten
    marks onto a baseline just to obtain one PDFium text object.
    """
    rows = []
    for glyph in glyphs:
        if not rows or abs(rows[-1][0]["py"] - glyph["py"]) > 1e-8:
            rows.append([])
        rows[-1].append(glyph)
    commands = []
    for row in rows:
        chunks = []
        for i, glyph in enumerate(row):
            chunks.append(f"<{glyph['cid']:04X}>")
            if i + 1 < len(row):
                adjustment = glyph["width"] + (glyph["px"] - row[i + 1]["px"]) * 1000 / size
                chunks.append(f"{adjustment:.12f}")
        commands += [f"BT {font_name} {size:.12f} Tf",
                     f"1 0 0 1 {row[0]['px']:.12f} {row[0]['py']:.12f} Tm",
                     "[" + " ".join(chunks) + "] TJ", "ET"]
    return commands


def generate(native, output, integer_widths=False):
    evidence = json.loads((native / "results.json").read_text(encoding="utf-8"))
    writer = PdfWriter()
    writer.pdf_header = "%PDF-1.7"
    specs = []
    for case in evidence["cases"]:
        source = native / case["pdf"]
        layout = json.loads(source.with_suffix(".layout.json").read_text(encoding="utf-8"))
        for mode in MODES:
            reader = PdfReader(source)
            page = writer.add_page(reader.pages[0])
            font_name, ref = next(iter(page["/Resources"]["/Font"].items()))
            font = ref.get_object()
            widths = font["/DescendantFonts"][0].get_object()["/W"][1]
            if integer_widths:
                widths = ArrayObject([NumberObject(round(float(width))) for width in widths])
                font["/DescendantFonts"][0].get_object()["/W"][1] = widths
            operations = ContentStream(page.get_contents(), writer).operations
            matrices = [args for args, op in operations if op == b"Tm"]
            cid = 0
            runs, clusters = [], []
            for run in layout["runs"]:
                glyphs, run_clusters = [], {}
                for g in run["glyphs"]:
                    cid += 1
                    start, end = g["cluster"]["utf8Start"], g["cluster"]["utf8End"]
                    logical = layout["text"].encode()[start:end].decode()
                    entry = dict(cid=cid, px=float(matrices[cid - 1][4]), py=float(matrices[cid - 1][5]),
                                 width=float(widths[cid - 1]), start=start, end=end, text=logical)
                    glyphs.append(entry)
                    run_clusters.setdefault(start, dict(start=start, text=logical, glyphs=[]))["glyphs"].append(entry)
                r = run["range"]
                runs.append(dict(start=r["utf8Start"], text=layout["text"].encode()[r["utf8Start"]:r["utf8End"]].decode(), glyphs=glyphs))
                clusters.extend(run_clusters.values())
            glyphs = [g for run in runs for g in run["glyphs"]]
            if "unicode" in mode or mode == "cluster_actual_logical":
                # Source-semantic mapping: each complete logical cluster is
                # assigned once to its first painted glyph; other glyphs map
                # to an explicit empty Unicode string, never a reversed one.
                mapping = {g["cid"]: "" for g in glyphs}
                for cluster in clusters:
                    mapping[cluster["glyphs"][0]["cid"]] = cluster["text"]
                cmap = ["/CIDInit /ProcSet findresource begin 12 dict begin begincmap",
                        "/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def",
                        "/CMapName /GroupedProbe def /CMapType 2 def",
                        "1 begincodespacerange <0000> <FFFF> endcodespacerange"]
                items = list(mapping.items())
                for offset in range(0, len(items), 100):
                    chunk = items[offset:offset + 100]
                    cmap += [f"{len(chunk)} beginbfchar", *[f"<{i:04X}> <{text.encode('utf-16-be').hex()}>" for i, text in chunk], "endbfchar"]
                cmap += ["endcmap CMapName currentdict /CMap defineresource pop end end"]
                font[NameObject("/ToUnicode")] = stream(writer, "\n".join(cmap).encode())
            if mode == "whole_grouped":
                groups = [dict(text=layout["text"], glyphs=glyphs)]
            elif mode.startswith("cluster"):
                groups = sorted(clusters, key=lambda group: group["start"]) if mode.endswith("logical") else clusters
            else:
                groups = sorted(runs, key=lambda group: group["start"]) if mode.endswith("logical") else runs
            commands = ["q 0.1 0.2 0.3 rg"]
            for group in groups:
                marked = "actual" in mode or mode == "whole_grouped"
                if marked:
                    commands.append(actual(group["text"]))
                commands += grouped_tj(group["glyphs"], layout["fontSize"], font_name)
                if marked:
                    commands.append("EMC")
            commands.append("Q")
            page[NameObject("/Contents")] = stream(writer, "\n".join(commands).encode())
            specs.append(dict(case=case["case"], rotation=case["rotation"], mode=mode,
                              source=layout["text"], original=str(source.resolve()),
                              expected_visible_bounds=layout["bounds"],
                              tj_objects=sum(command.endswith("] TJ") for command in commands)))
    writer.write(output)
    return specs


class Pdfium:
    def __init__(self, path):
        self.lib = c.WinDLL(str(path.resolve()))
        pointer, integer, double = c.c_void_p, c.c_int, c.c_double
        signatures = {
            "FPDF_InitLibrary": ([], None), "FPDF_DestroyLibrary": ([], None),
            "FPDF_LoadDocument": ([c.c_char_p, c.c_char_p], pointer), "FPDF_CloseDocument": ([pointer], None),
            "FPDF_GetPageCount": ([pointer], integer), "FPDF_LoadPage": ([pointer, integer], pointer),
            "FPDF_ClosePage": ([pointer], None), "FPDFText_LoadPage": ([pointer], pointer),
            "FPDFText_ClosePage": ([pointer], None), "FPDFText_CountChars": ([pointer], integer),
            "FPDFText_GetText": ([pointer, integer, integer, c.POINTER(c.c_ushort)], integer),
            "FPDFText_GetUnicode": ([pointer, integer], c.c_uint),
            "FPDFText_GetCharBox": ([pointer, integer, *([c.POINTER(double)] * 4)], integer),
            "FPDF_SaveAsCopy": ([pointer, pointer, c.c_ulong], integer),
        }
        for symbol, (arguments, result) in signatures.items():
            getattr(self.lib, symbol).argtypes, getattr(self.lib, symbol).restype = arguments, result
        self.lib.FPDF_InitLibrary()

    def inspect(self, path, save=None):
        lib = self.lib
        doc = lib.FPDF_LoadDocument(str(path.resolve()).encode(), None)
        assert doc
        pages = []
        try:
            for i in range(lib.FPDF_GetPageCount(doc)):
                page = lib.FPDF_LoadPage(doc, i)
                text_page = lib.FPDFText_LoadPage(page)
                try:
                    count = lib.FPDFText_CountChars(text_page)
                    buffer = (c.c_ushort * (2 * count + 1))()
                    written = lib.FPDFText_GetText(text_page, 0, count, buffer)
                    text = bytes(buffer)[:max(0, written - 1) * 2].decode("utf-16-le")
                    chars = []
                    for j in range(count):
                        box = [c.c_double() for _ in range(4)]
                        valid = lib.FPDFText_GetCharBox(text_page, j, *(c.byref(v) for v in box))
                        chars.append(dict(unicode=lib.FPDFText_GetUnicode(text_page, j),
                                          box=[v.value for v in box] if valid else None))
                    pages.append(dict(text=text, chars=chars))
                finally:
                    lib.FPDFText_ClosePage(text_page)
                    lib.FPDF_ClosePage(page)
            if save:
                chunks = []
                callback_type = c.CFUNCTYPE(c.c_int, c.c_void_p, c.c_void_p, c.c_ulong)
                class Writer(c.Structure):
                    _fields_ = [("version", c.c_int), ("WriteBlock", callback_type)]
                def write(_self, data, size):
                    chunks.append(c.string_at(data, size))
                    return 1
                callback = callback_type(write)
                handle = Writer(1, callback)
                assert lib.FPDF_SaveAsCopy(doc, c.byref(handle), 0)
                save.write_bytes(b"".join(chunks))
        finally:
            lib.FPDF_CloseDocument(doc)
        return pages


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, default=ROOT / "artifacts/shaped-text/native")
    parser.add_argument("--output", type=Path, default=ROOT / "artifacts/shaped-text/grouped")
    parser.add_argument("--pdfium", type=Path, default=ROOT / "src-tauri/resources/pdfium/pdfium.dll")
    parser.add_argument("--integer-widths", action="store_true", help="Emit integral W entries and compensate positions in TJ, avoiding reader fractional-width quantization")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    pdf = args.output / "grouped.pdf"
    resaved = args.output / "grouped.resaved.pdf"
    specs = generate(args.input, pdf, args.integer_widths)
    pdfium = Pdfium(args.pdfium)
    try:
        text_before = pdfium.inspect(pdf, resaved)
        text_after = pdfium.inspect(resaved)
    finally:
        pdfium.lib.FPDF_DestroyLibrary()
    source_cache = {}
    results = []
    before_reader, after_reader = PdfReader(pdf), PdfReader(resaved)
    with fitz.open(pdf) as before, fitz.open(resaved) as after:
        for i, spec in enumerate(specs):
            if spec["original"] not in source_cache:
                with fitz.open(spec["original"]) as original:
                    original_pixmap = original[0].get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False)
                    source_cache[spec["original"]] = original_pixmap.samples
            original_samples = source_cache[spec["original"]]
            before_pixels = before[i].get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False).samples
            after_pixels = after[i].get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False).samples
            assert len(before_pixels) == len(original_samples)
            changed = sum(a != b for a, b in zip(before_pixels, original_samples))
            max_delta = max((abs(a - b) for a, b in zip(before_pixels, original_samples)), default=0)
            raw = dict(pdfium=text_before[i]["text"], mupdf=before[i].get_text(), pypdf=before_reader.pages[i].extract_text())
            raw_after = dict(pdfium=text_after[i]["text"], mupdf=after[i].get_text(), pypdf=after_reader.pages[i].extract_text())
            results.append(dict(spec, raw=raw, raw_resaved=raw_after,
                                exact={reader: text.rstrip("\r\n") == spec["source"] for reader, text in raw.items()},
                                pdfium_characters=text_before[i]["chars"],
                                pdfium_geometry_unchanged=text_before[i]["chars"] == text_after[i]["chars"],
                                raw_unchanged=raw == raw_after, raster_unchanged=before_pixels == after_pixels,
                                original_raster_sha256=sha(original_samples), raster_sha256=sha(before_pixels),
                                changed_rgb_channels_vs_native=changed, max_channel_delta_vs_native=max_delta))
    report = dict(versions={name: importlib.metadata.version(name) for name in ("pymupdf", "pypdf")},
                  pdfium_sha256=sha(args.pdfium.read_bytes()), integer_widths=args.integer_widths, results=results,
                  limitation="Research modifies only PDF grouping/maps. Native HarfRust layout unchanged. Empty ToUnicode destinations are a compatibility experiment, not a recommended production encoding.")
    (args.output / "results.json").write_text(json.dumps(report, ensure_ascii=True, indent=2), encoding="utf-8")
    for mode in MODES:
        rows = [r for r in results if r["mode"] == mode]
        print(json.dumps(dict(mode=mode, exact={reader: sum(row["exact"][reader] for row in rows) for reader in ("pdfium", "mupdf", "pypdf")},
                              cases=len(rows), identical_native_raster=sum(row["changed_rgb_channels_vs_native"] == 0 for row in rows),
                              all_save_unchanged=all(row["raw_unchanged"] and row["raster_unchanged"] for row in rows))))
    print(f"Saved {args.output / 'results.json'}")


if __name__ == "__main__":
    main()
