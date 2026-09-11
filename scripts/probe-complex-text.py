"""Issue #13 research only: shape actual fonts and measure PDF extraction.

Install isolated research dependencies from the repository root:
  python -m pip install --target artifacts/complex-text/python \
      uharfbuzz==0.55.0 python-bidi==0.6.11 fonttools==4.62.1 \
      pypdf==6.14.2 pymupdf==1.27.1
Then run: python scripts/probe-complex-text.py

The PDF experiment embeds only the repository's licensed DejaVu fixture. System
Arabic/Indic fonts are read for shaping, never copied into the output PDF.
This is neither a production layout engine nor a Unicode conformance suite.
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import importlib.metadata
import json
from pathlib import Path
import platform
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "artifacts/complex-text/python"))

import uharfbuzz as hb
from bidi import algorithm as bidi
from fontTools.ttLib import TTFont
import fitz
from pypdf import PdfReader, PdfWriter
from pypdf.generic import (
    ArrayObject, DecodedStreamObject, DictionaryObject, FloatObject,
    NameObject, NumberObject, TextStringObject,
)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def font_input(path, index=0):
    data = path.read_bytes()
    parsed = TTFont(path, fontNumber=index, lazy=True)
    face = hb.Face(data, index)
    font = hb.Font(face)
    font.scale = (face.upem, face.upem)
    hb.ot_font_set_funcs(font)
    metadata = {
        "path": str(path.resolve()), "face_index": index,
        "sha256": digest(data), "bytes": len(data), "upem": face.upem,
        "family": parsed["name"].getDebugName(1),
        "fs_type": parsed["OS/2"].fsType,
    }
    parsed.close()
    return {"font": font, "bytes": data, "metadata": metadata}


def utf16_offsets(text):
    offsets = [0]
    for char in text:
        offsets.append(offsets[-1] + len(char.encode("utf-16-le")) // 2)
    return offsets


def shape(asset, text, direction="ltr", script="Latn", language="en", liga=True):
    buffer = hb.Buffer()
    buffer.add_codepoints([ord(char) for char in text])
    buffer.direction, buffer.script, buffer.language = direction, script, language
    buffer.cluster_level = hb.BufferClusterLevel.MONOTONE_CHARACTERS
    hb.shape(asset["font"], buffer, {"liga": liga})
    infos, positions = buffer.glyph_infos, buffer.glyph_positions
    starts = sorted({info.cluster for info in infos} | {len(text)})
    ends = dict(zip(starts, starts[1:]))
    offsets = utf16_offsets(text)
    glyphs = []
    for info, pos in zip(infos, positions):
        start, end = info.cluster, ends[info.cluster]
        glyphs.append({
            "gid": info.codepoint, "cluster_scalar": [start, end],
            "cluster_utf16": [offsets[start], offsets[end]],
            "cluster_text": text[start:end],
            "x_advance": pos.x_advance, "y_advance": pos.y_advance,
            "x_offset": pos.x_offset, "y_offset": pos.y_offset,
        })
    return {
        "text": text, "direction": direction, "script": script,
        "language": language, "liga": liga, "cluster_level": 1,
        "glyphs": glyphs, "scalar_count": len(text),
        "glyph_count": len(glyphs), "missing_glyphs": sum(g["gid"] == 0 for g in glyphs),
        "advance_em": sum(g["x_advance"] for g in glyphs) / asset["metadata"]["upem"],
    }


def mixed_bidi(asset, text):
    """Simple legacy bidi probe; deliberately no isolates or script itemizer."""
    storage = bidi.get_empty_storage()
    storage["base_level"] = bidi.get_base_level(text)
    storage["base_dir"] = ("L", "R")[storage["base_level"]]
    bidi.get_embedding_levels(text, storage)
    for index, char in enumerate(storage["chars"]):
        char["source_index"] = index
    bidi.explicit_embed_and_overrides(storage)
    bidi.resolve_weak_types(storage)
    bidi.resolve_neutral_types(storage, False)
    bidi.resolve_implicit_levels(storage, False)
    levels = [char["level"] for char in storage["chars"]]
    runs = []
    for index, level in enumerate(levels):
        if not runs or runs[-1]["level"] != level:
            runs.append({"start": index, "end": index + 1, "level": level})
        else:
            runs[-1]["end"] = index + 1
    bidi.reorder_resolved_levels(storage, False)
    order = []
    for char in storage["chars"]:
        run_index = next(i for i, run in enumerate(runs)
                         if run["start"] <= char["source_index"] < run["end"])
        if run_index not in order:
            order.append(run_index)
    for run in runs:
        rtl = run["level"] % 2 == 1
        run["shape"] = shape(asset, text[run["start"]:run["end"]],
                             "rtl" if rtl else "ltr", "Arab" if rtl else "Latn",
                             "ar" if rtl else "en")
    return {"text": text, "base_level": storage["base_level"],
            "levels": levels, "logical_runs": runs, "visual_run_order": order,
            "visual_scalar_indices": [c["source_index"] for c in storage["chars"]],
            "limitation": "Legacy Python bidi algorithm; this sample has no isolates. "
                          "Level-based script choice is valid only for this fixed sample."}


def dictionary(**items):
    return DictionaryObject({NameObject("/" + key): value for key, value in items.items()})


def name(value):
    return NameObject("/" + value)


def numbers(values):
    return ArrayObject([FloatObject(value) for value in values])


def add_stream(writer, data, **items):
    stream = DecodedStreamObject()
    stream.set_data(data)
    stream.update(dictionary(**items))
    return writer._add_object(stream)


def pdf_probe(asset, cases, path):
    """Two deliberately simple semantic strategies, not an export solution.

    Assign a CID per glyph occurrence and map each to its complete HB cluster.
    This knowingly duplicates Unicode for multi-glyph clusters. Compare that
    negative control with /ActualText over the whole logical run.
    """
    writer = PdfWriter()
    writer.pdf_header = "%PDF-1.7"
    parsed = TTFont(ROOT / "tests/fixtures/corpus/fonts/DejaVuSerif.ttf")
    units = asset["metadata"]["upem"]
    scale = 1000 / units
    head, hhea = parsed["head"], parsed["hhea"]
    program = add_stream(writer, asset["bytes"], Length1=NumberObject(len(asset["bytes"])))
    descriptor = writer._add_object(dictionary(
        Type=name("FontDescriptor"), FontName=name("DejaVuSerif"), Flags=NumberObject(32),
        FontBBox=numbers([head.xMin * scale, head.yMin * scale, head.xMax * scale, head.yMax * scale]),
        ItalicAngle=NumberObject(0), Ascent=FloatObject(hhea.ascent * scale),
        Descent=FloatObject(hhea.descent * scale), CapHeight=FloatObject(hhea.ascent * scale),
        StemV=NumberObject(80), FontFile2=program,
    ))
    page_specs = []
    for case_id, case in cases.items():
        for actual_text in (False, True):
            glyphs = case["glyphs"]
            mapping = b"\0\0" + b"".join(g["gid"].to_bytes(2, "big") for g in glyphs)
            cmap = ["/CIDInit /ProcSet findresource begin", "12 dict begin begincmap",
                    "/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def",
                    "/CMapName /Research def /CMapType 2 def",
                    "1 begincodespacerange <0000> <FFFF> endcodespacerange",
                    f"{len(glyphs)} beginbfchar"]
            cmap += [f"<{index:04X}> <{glyph['cluster_text'].encode('utf-16-be').hex()}>"
                     for index, glyph in enumerate(glyphs, 1)]
            cmap += ["endbfchar endcmap CMapName currentdict /CMap defineresource pop end end"]
            widths = [parsed["hmtx"].metrics[parsed.getGlyphName(g["gid"])][0] * scale for g in glyphs]
            descendant = writer._add_object(dictionary(
                Type=name("Font"), Subtype=name("CIDFontType2"), BaseFont=name("DejaVuSerif"),
                CIDSystemInfo=dictionary(Registry=TextStringObject("Adobe"),
                                         Ordering=TextStringObject("Identity"), Supplement=NumberObject(0)),
                FontDescriptor=descriptor, DW=NumberObject(0),
                W=ArrayObject([NumberObject(1), numbers(widths)]),
                CIDToGIDMap=add_stream(writer, mapping),
            ))
            font = writer._add_object(dictionary(
                Type=name("Font"), Subtype=name("Type0"), BaseFont=name("DejaVuSerif"),
                Encoding=name("Identity-H"), DescendantFonts=ArrayObject([descendant]),
                ToUnicode=add_stream(writer, "\n".join(cmap).encode("ascii")),
            ))
            page = writer.add_blank_page(width=480, height=160)
            page[name("Resources")] = dictionary(Font=dictionary(F0=font))
            commands = ["q"]
            if actual_text:
                logical = ("\ufeff" + case["text"]).encode("utf-16-be").hex()
                commands.append(f"/Span << /ActualText <{logical}> >> BDC")
            commands.append("BT /F0 36 Tf")
            x, y = 0, 0
            for index, glyph in enumerate(glyphs, 1):
                px = 32 + (x + glyph["x_offset"]) * 36 / units
                py = 80 + (y + glyph["y_offset"]) * 36 / units
                commands.append(f"1 0 0 1 {px:.6f} {py:.6f} Tm <{index:04X}> Tj")
                x += glyph["x_advance"]
                y += glyph["y_advance"]
            commands.append("ET")
            if actual_text:
                commands.append("EMC")
            commands.append("Q")
            page[name("Contents")] = add_stream(writer, "\n".join(commands).encode("ascii"))
            page_specs.append({"case": case_id, "source": case["text"], "actual_text": actual_text})
    writer.write(path)
    parsed.close()
    return page_specs


def pdfium_probe(dll_path, pdf_path):
    if not dll_path.exists() or sys.platform != "win32":
        return {"status": "skipped", "reason": "bundled Windows DLL unavailable"}
    library = ctypes.WinDLL(str(dll_path.resolve()))
    exported = {symbol: hasattr(library, symbol) for symbol in (
        "FPDFText_SetText", "FPDFText_SetCharcodes", "FPDFText_SetPositions",
        "FPDFText_LoadCidType2Font", "FPDFPageObj_CreateTextObj", "FPDFPageObj_SetMatrix",
        "FPDFFormObj_CountObjects", "FPDFFormObj_GetObject", "FPDFFont_GetFontData",
    )}
    pointer, integer = ctypes.c_void_p, ctypes.c_int
    signatures = {
        "FPDF_InitLibrary": ([], None), "FPDF_DestroyLibrary": ([], None),
        "FPDF_LoadDocument": ([ctypes.c_char_p, ctypes.c_char_p], pointer),
        "FPDF_GetPageCount": ([pointer], integer),
        "FPDF_LoadPage": ([pointer, integer], pointer),
        "FPDFText_LoadPage": ([pointer], pointer),
        "FPDFText_CountChars": ([pointer], integer),
        "FPDFText_GetText": ([pointer, integer, integer, ctypes.POINTER(ctypes.c_ushort)], integer),
        "FPDFText_ClosePage": ([pointer], None), "FPDF_ClosePage": ([pointer], None),
        "FPDF_CloseDocument": ([pointer], None),
    }
    for symbol, (arguments, result) in signatures.items():
        function = getattr(library, symbol)
        function.argtypes, function.restype = arguments, result
    library.FPDF_InitLibrary()
    document = library.FPDF_LoadDocument(str(pdf_path.resolve()).encode("utf-8"), None)
    if not document:
        library.FPDF_DestroyLibrary()
        raise RuntimeError("PDFium failed to open research PDF")
    pages = []
    try:
        for index in range(library.FPDF_GetPageCount(document)):
            page = library.FPDF_LoadPage(document, index)
            if not page:
                raise RuntimeError(f"PDFium failed to load page {index}")
            text_page = None
            try:
                text_page = library.FPDFText_LoadPage(page)
                if not text_page:
                    raise RuntimeError("PDFium failed to load text")
                count = library.FPDFText_CountChars(text_page)
                buffer = (ctypes.c_ushort * (count + 1))()
                written = library.FPDFText_GetText(text_page, 0, count, buffer)
                pages.append(bytes(buffer)[:max(0, written - 1) * 2].decode("utf-16-le"))
            finally:
                if text_page:
                    library.FPDFText_ClosePage(text_page)
                library.FPDF_ClosePage(page)
    finally:
        library.FPDF_CloseDocument(document)
        library.FPDF_DestroyLibrary()
    return {"status": "measured", "dll_path": str(dll_path.resolve()),
            "dll_sha256": digest(dll_path.read_bytes()), "exports": exported,
            "pages": pages,
            "limitation": "Exports verified; setter APIs not executed by this probe."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "artifacts/complex-text")
    parser.add_argument("--arabic-font", type=Path, default=Path("C:/Windows/Fonts/tahoma.ttf"))
    parser.add_argument("--indic-font", type=Path, default=Path("C:/Windows/Fonts/Nirmala.ttc"))
    parser.add_argument("--indic-index", type=int, default=0)
    parser.add_argument("--pdfium", type=Path, default=ROOT / "src-tauri/resources/pdfium/pdfium.dll")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    latin = font_input(ROOT / "tests/fixtures/corpus/fonts/DejaVuSerif.ttf")
    result = {"purpose": "Research only; not app support or Unicode conformance",
              "python": platform.python_version(), "harfbuzz": hb.version_string(),
              "packages": {package: importlib.metadata.version(package)
                           for package in ("uharfbuzz", "python-bidi", "fonttools", "pypdf", "pymupdf")},
              "fonts": {"latin": latin["metadata"]}, "cases": {}, "skipped": []}
    for key, text, liga in (
        ("ligature_on", "office", True), ("ligature_off", "office", False),
        ("composed", "caf\u00e9", True), ("decomposed", "cafe\u0301", True),
        ("mark_offsets", "q\u0307\u0323", True),
    ):
        result["cases"][key] = shape(latin, text, liga=liga)
    for key, path, index, text, direction, script, language in (
        ("arabic", args.arabic_font, 0, "\u0633\u0644\u0627\u0645", "rtl", "Arab", "ar"),
        ("indic", args.indic_font, args.indic_index, "\u0915\u093f\u0924\u093e\u092c", "ltr", "Deva", "hi"),
    ):
        if not path.exists():
            result["skipped"].append({"case": key, "reason": f"font not found: {path}"})
            continue
        asset = font_input(path, index)
        result["fonts"][key] = asset["metadata"]
        result["cases"][key] = shape(asset, text, direction, script, language)
        if key == "arabic":
            result["mixed_bidi"] = mixed_bidi(asset, "ABC \u0633\u0644\u0627\u0645 123 DEF")
    pdf_path = args.output / "cluster-extraction.pdf"
    pages = pdf_probe(latin, {key: result["cases"][key]
                             for key in ("ligature_on", "decomposed", "mark_offsets")}, pdf_path)
    result["pdfium"] = pdfium_probe(args.pdfium, pdf_path)
    independent = PdfReader(pdf_path)
    raster_hashes = []
    with fitz.open(pdf_path) as mupdf:
        for index, spec in enumerate(pages):
            outputs = {"pypdf": independent.pages[index].extract_text(),
                       "mupdf": mupdf[index].get_text()}
            if result["pdfium"]["status"] == "measured":
                outputs["pdfium"] = result["pdfium"]["pages"][index]
            spec["extracted"] = outputs
            spec["exact_after_trailing_newlines"] = {
                reader: value.rstrip("\r\n") == spec["source"] for reader, value in outputs.items()
            }
            pixmap = mupdf[index].get_pixmap(matrix=fitz.Matrix(2, 2))
            raster_hashes.append(digest(pixmap.samples))
            pixmap.save(
                args.output / f"page-{index + 1}-{spec['case']}-actual-{spec['actual_text']}.png")
    result["pdf_experiment"] = {"path": str(pdf_path), "sha256": digest(pdf_path.read_bytes()),
                                "strategy": "Full cluster per glyph (negative control), with/without ActualText",
                                "pages": pages,
                                "actual_text_preserves_mupdf_pixels": all(
                                    raster_hashes[i] == raster_hashes[i + 1]
                                    for i in range(0, len(raster_hashes), 2))}
    # Assertions validate probe setup. Reader disagreements are observations.
    assert all(case["missing_glyphs"] == 0 for case in result["cases"].values())
    assert result["cases"]["ligature_on"]["glyph_count"] < result["cases"]["ligature_off"]["glyph_count"]
    assert any(g["x_offset"] or g["y_offset"] for g in result["cases"]["mark_offsets"]["glyphs"])
    assert all(g["cluster_scalar"][0] < g["cluster_scalar"][1]
               for case in result["cases"].values() for g in case["glyphs"])
    assert result["pdf_experiment"]["actual_text_preserves_mupdf_pixels"]
    if "mixed_bidi" in result:
        assert result["mixed_bidi"]["visual_run_order"] == [0, 2, 1, 3]
        assert all(run["shape"]["missing_glyphs"] == 0 for run in result["mixed_bidi"]["logical_runs"])
    (args.output / "results.json").write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"cases": {key: {field: case[field] for field in ("scalar_count", "glyph_count", "missing_glyphs")}
                                for key, case in result["cases"].items()},
                      "reader_checks": [{"case": p["case"], "actual_text": p["actual_text"],
                                         "exact": p["exact_after_trailing_newlines"]} for p in pages],
                      "output": str(args.output)}, indent=2))


if __name__ == "__main__":
    main()
