"""Development-only #14 PDF interoperability gate; does not exercise Folio layout.

Install dependencies listed in probe-complex-text.py, or pass --python-deps with
that isolated target directory. Output includes raw reader strings and PDFium
character boxes before and after FPDF_SaveAsCopy. No text normalization is used.
"""
from __future__ import annotations

import argparse
import ctypes as c
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--python-deps", type=Path, default=ROOT / "artifacts/complex-text/python")
parser.add_argument("--output", type=Path, default=ROOT / "artifacts/shaped-text/interop")
parser.add_argument("--pdfium", type=Path, default=ROOT / "src-tauri/resources/pdfium/pdfium.dll")
parser.add_argument("--inspect", type=Path, help="Inspect an existing native-produced PDF instead of generating research PDFs")
args = parser.parse_args()
sys.path.insert(0, str(args.python_deps))
spec = importlib.util.spec_from_file_location("research", ROOT / "scripts/probe-complex-text.py")
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)


def generate(path):
    writer = p.PdfWriter()
    writer.pdf_header = "%PDF-1.7"
    fixture = ROOT / "tests/fixtures/shaped-text"
    serif = ROOT / "tests/fixtures/corpus/fonts/DejaVuSerif.ttf"
    cases = [
        ("ligature", serif, "office", "ltr", "Latn", "en"),
        ("marks", serif, "q\u0307\u0323", "ltr", "Latn", "en"),
        ("supplementary", serif, "A\U0001d434B", "ltr", "Latn", "en"),
        ("arabic", fixture / "NotoSansArabic-Regular.ttf", "\u0633\u0644\u0627\u0645", "rtl", "Arab", "ar"),
        ("indic", fixture / "NotoSansDevanagari-Regular.ttf", "\u0915\u093f\u0924\u093e\u092c", "ltr", "Deva", "hi"),
        ("mixed", fixture / "DejaVuSans.ttf", "ABC \u0633\u0644\u0627\u0645 123 DEF", "mixed", "", ""),
    ]
    pages = []
    for case_id, font_path, logical, direction, script, language in cases:
        asset = p.font_input(font_path)
        if direction == "mixed":
            bidi = p.mixed_bidi(asset, logical)
            runs = [dict(text=r["shape"]["text"], glyphs=r["shape"]["glyphs"], logical_index=i)
                    for i in bidi["visual_run_order"] for r in [bidi["logical_runs"][i]]]
        else:
            shaped = p.shape(asset, logical, direction, script, language)
            runs = [dict(text=logical, glyphs=shaped["glyphs"], logical_index=0)]
        glyphs = [g for run in runs for g in run["glyphs"]]
        assert all(g["gid"] for g in glyphs), (case_id, "unexpected missing glyph")
        parsed = p.TTFont(font_path)
        units = asset["metadata"]["upem"]
        scale = 1000 / units
        head, hhea = parsed["head"], parsed["hhea"]
        font_name = parsed["name"].getDebugName(6)
        program = p.add_stream(writer, asset["bytes"], Length1=p.NumberObject(len(asset["bytes"])))
        descriptor = writer._add_object(p.dictionary(
            Type=p.name("FontDescriptor"), FontName=p.name(font_name), Flags=p.NumberObject(32),
            FontBBox=p.numbers([head.xMin * scale, head.yMin * scale, head.xMax * scale, head.yMax * scale]),
            ItalicAngle=p.NumberObject(0), Ascent=p.FloatObject(hhea.ascent * scale),
            Descent=p.FloatObject(hhea.descent * scale), CapHeight=p.FloatObject(hhea.ascent * scale),
            StemV=p.NumberObject(80), FontFile2=program))
        mapping = b"\0\0" + b"".join(g["gid"].to_bytes(2, "big") for g in glyphs)
        cmap = ["/CIDInit /ProcSet findresource begin 12 dict begin begincmap",
                "/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def",
                "/CMapName /Research def /CMapType 2 def",
                "1 begincodespacerange <0000> <FFFF> endcodespacerange", f"{len(glyphs)} beginbfchar"]
        cmap += [f"<{i:04X}> <{g['cluster_text'].encode('utf-16-be').hex()}>" for i, g in enumerate(glyphs, 1)]
        cmap.append("endbfchar endcmap CMapName currentdict /CMap defineresource pop end end")
        widths = [parsed["hmtx"].metrics[parsed.getGlyphName(g["gid"])][0] * scale for g in glyphs]
        descendant = writer._add_object(p.dictionary(
            Type=p.name("Font"), Subtype=p.name("CIDFontType2"), BaseFont=p.name(font_name),
            CIDSystemInfo=p.dictionary(Registry=p.TextStringObject("Adobe"), Ordering=p.TextStringObject("Identity"), Supplement=p.NumberObject(0)),
            FontDescriptor=descriptor, DW=p.NumberObject(0),
            W=p.ArrayObject([p.NumberObject(1), p.numbers(widths)]), CIDToGIDMap=p.add_stream(writer, mapping)))
        font = writer._add_object(p.dictionary(
            Type=p.name("Font"), Subtype=p.name("Type0"), BaseFont=p.name(font_name),
            Encoding=p.name("Identity-H"), DescendantFonts=p.ArrayObject([descendant]),
            ToUnicode=p.add_stream(writer, "\n".join(cmap).encode("ascii"))))
        placed = []
        x = y = 0
        cid = 0
        for run in runs:
            positions = []
            for g in run["glyphs"]:
                cid += 1
                px = 32 + (x + g["x_offset"]) * 36 / units
                py = 80 + (y + g["y_offset"]) * 36 / units
                positions.append(f"1 0 0 1 {px:.6f} {py:.6f} Tm <{cid:04X}> Tj")
                x += g["x_advance"]
                y += g["y_advance"]
            placed.append(dict(run, positions=positions))
        cluster_groups = []
        cluster_counts = {}
        for run in placed:
            groups = {}
            for g, command in zip(run["glyphs"], run["positions"]):
                start, end = g["cluster_scalar"]
                group = groups.setdefault(start, dict(text=g["cluster_text"], positions=[], logical_index=run["logical_index"], start=start))
                group["positions"].append(command)
            cluster_groups.extend(groups.values())
            for start, group in groups.items():
                cluster_counts[(run["logical_index"], start)] = len(group["positions"])
        for mode in ("none", "whole", "visual_runs", "logical_runs", "logical_clusters"):
            commands = ["q"]
            def actual(text):
                encoded = ("\ufeff" + text).encode("utf-16-be").hex()
                return f"/Span << /ActualText <{encoded}> >> BDC"
            if mode == "whole":
                commands.append(actual(logical))
            page_font = font
            if mode == "logical_clusters":
                # Map only single-glyph clusters in this exploratory variant.
                # Multi-glyph cluster semantics come only from ActualText.
                mappings = []
                i = 0
                for run in placed:
                    for glyph in run["glyphs"]:
                        i += 1
                        if cluster_counts[(run["logical_index"], glyph["cluster_scalar"][0])] == 1:
                            mappings.append(f"<{i:04X}> <{glyph['cluster_text'].encode('utf-16-be').hex()}>")
                cluster_cmap = cmap[:4] + [f"{len(mappings)} beginbfchar", *mappings, cmap[-1]]
                page_font = writer._add_object(p.dictionary(
                    Type=p.name("Font"), Subtype=p.name("Type0"), BaseFont=p.name(font_name),
                    Encoding=p.name("Identity-H"), DescendantFonts=p.ArrayObject([descendant]),
                    ToUnicode=p.add_stream(writer, "\n".join(cluster_cmap).encode("ascii"))))
                ordered = sorted(cluster_groups, key=lambda r: (r["logical_index"], r["start"]))
            else:
                ordered = sorted(placed, key=lambda r: r["logical_index"]) if mode == "logical_runs" else placed
            for run in ordered:
                if mode.endswith("runs") or mode == "logical_clusters":
                    commands.append(actual(run["text"]))
                commands.extend(["BT /F0 36 Tf", *run["positions"], "ET"])
                if mode.endswith("runs") or mode == "logical_clusters":
                    commands.append("EMC")
            if mode == "whole":
                commands.append("EMC")
            commands.append("Q")
            page = writer.add_blank_page(width=640, height=160)
            page[p.name("Resources")] = p.dictionary(Font=p.dictionary(F0=page_font))
            page[p.name("Contents")] = p.add_stream(writer, "\n".join(commands).encode("ascii"))
            pages.append(dict(case=case_id, source=logical, mode=mode, font=asset["metadata"], runs=runs))
        parsed.close()
    writer.write(path)
    return pages


def pdfium_read_and_save(source, resaved):
    lib = c.WinDLL(str(args.pdfium.resolve()))
    ptr, integer, double = c.c_void_p, c.c_int, c.c_double
    signatures = {
        "FPDF_InitLibrary": ([], None), "FPDF_DestroyLibrary": ([], None),
        "FPDF_LoadDocument": ([c.c_char_p, c.c_char_p], ptr), "FPDF_CloseDocument": ([ptr], None),
        "FPDF_GetPageCount": ([ptr], integer), "FPDF_LoadPage": ([ptr, integer], ptr), "FPDF_ClosePage": ([ptr], None),
        "FPDFText_LoadPage": ([ptr], ptr), "FPDFText_ClosePage": ([ptr], None),
        "FPDFText_CountChars": ([ptr], integer),
        "FPDFText_GetText": ([ptr, integer, integer, c.POINTER(c.c_ushort)], integer),
        "FPDFText_GetUnicode": ([ptr, integer], c.c_uint),
        "FPDFText_GetCharBox": ([ptr, integer, *([c.POINTER(double)] * 4)], integer),
        "FPDF_SaveAsCopy": ([ptr, ptr, c.c_ulong], integer),
    }
    for symbol, (arguments, result) in signatures.items():
        getattr(lib, symbol).argtypes, getattr(lib, symbol).restype = arguments, result
    callback_type = c.CFUNCTYPE(integer, ptr, ptr, c.c_ulong)
    class FileWrite(c.Structure):
        _fields_ = [("version", integer), ("WriteBlock", callback_type)]
    lib.FPDF_InitLibrary()
    def read(path, save=None):
        doc = lib.FPDF_LoadDocument(str(path.resolve()).encode("utf-8"), None)
        assert doc, path
        result = []
        try:
            for i in range(lib.FPDF_GetPageCount(doc)):
                page = lib.FPDF_LoadPage(doc, i)
                tp = lib.FPDFText_LoadPage(page)
                try:
                    count = lib.FPDFText_CountChars(tp)
                    buffer = (c.c_ushort * (2 * count + 1))()
                    written = lib.FPDFText_GetText(tp, 0, count, buffer)
                    text = bytes(buffer)[:max(0, written - 1) * 2].decode("utf-16-le")
                    chars = []
                    for j in range(count):
                        box = [double() for _ in range(4)]
                        valid = lib.FPDFText_GetCharBox(tp, j, *(c.byref(v) for v in box))
                        chars.append(dict(unicode=lib.FPDFText_GetUnicode(tp, j), box=[v.value for v in box] if valid else None))
                    result.append(dict(text=text, characters=chars))
                finally:
                    lib.FPDFText_ClosePage(tp)
                    lib.FPDF_ClosePage(page)
            if save:
                chunks = []
                def write(_self, data, size):
                    chunks.append(c.string_at(data, size))
                    return 1
                callback = callback_type(write)
                handler = FileWrite(1, callback)
                assert lib.FPDF_SaveAsCopy(doc, c.byref(handler), 0)
                save.write_bytes(b"".join(chunks))
        finally:
            lib.FPDF_CloseDocument(doc)
        return result
    try:
        return dict(original=read(source, resaved), resaved=read(resaved), dll_sha256=hashlib.sha256(args.pdfium.read_bytes()).hexdigest())
    finally:
        lib.FPDF_DestroyLibrary()


def independent(path):
    with p.fitz.open(path) as doc:
        mupdf = [page.get_text() for page in doc]
        raster_hashes = [hashlib.sha256(page.get_pixmap(matrix=p.fitz.Matrix(2, 2), alpha=False).samples).hexdigest() for page in doc]
    return dict(mupdf=mupdf, pypdf=[page.extract_text() for page in p.PdfReader(path).pages], raster_sha256=raster_hashes)


args.output.mkdir(parents=True, exist_ok=True)
source = args.inspect or args.output / "shaped-clusters.pdf"
pages = None if args.inspect else generate(source)
saved = args.output / "pdfium-resaved.pdf"
result = dict(source=str(source.resolve()), pages=pages,
              pdfium=pdfium_read_and_save(source, saved),
              original=independent(source), resaved=independent(saved),
              versions={name: p.importlib.metadata.version(name) for name in ("uharfbuzz", "python-bidi", "fonttools", "pypdf", "pymupdf")},
              limitation="Research shaping uses legacy python-bidi only for fixed mixed sample; not production layout or Unicode conformance. PDFium save does not regenerate page content.")
(args.output / "results.json").write_text(json.dumps(result, ensure_ascii=True, indent=2), encoding="utf-8")
for i, page in enumerate(pages or [{} for _ in result["original"]["mupdf"]]):
    print(json.dumps(dict(page=i, case=page.get("case"), mode=page.get("mode"), source=page.get("source"),
                         pdfium=result["pdfium"]["original"][i]["text"], mupdf=result["original"]["mupdf"][i],
                         pypdf=result["original"]["pypdf"][i]), ensure_ascii=True))
print(f"Saved evidence: {args.output / 'results.json'}")
