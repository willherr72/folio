"""Compare nested text-state inheritance using an explicitly selected PDFium DLL.

No Folio code or PDFium page import is involved. Generated PDFs contain only
original synthetic text. Requires pypdf, PyMuPDF and Pillow. A nonzero exit means
the renderers disagree with each document's explicit-state reference; use
--expect-known-defects only to pin the currently bundled renderer's diagnosis.
"""

import argparse
import ctypes as c
import hashlib
import json
from pathlib import Path

import fitz
from PIL import Image, ImageChops
from pypdf import PdfWriter
from pypdf.generic import (
    ArrayObject, DecodedStreamObject, DictionaryObject, NameObject, NumberObject,
)


CASES = {
    "leading": ("17 TL", "T*", True),
    "rise": ("9 Ts", "", True),
    "horizontal-scale": ("65 Tz", "", True),
    "character-spacing": ("2 Tc", "", False),
    "word-spacing": ("5 Tw", "", False),
    "font-size": ("/F1 23 Tf", "", False),
    "fill-color": ("0.1 0.5 0.8 rg", "", False),
    "stroke-mode": ("1 Tr 0.7 w", "", False),
    "combined": ("17 TL 9 Ts 65 Tz 2 Tc 5 Tw", "T*", True),
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fixture(path, state, position, explicit, nested):
    writer = PdfWriter()
    page = writer.add_blank_page(width=320, height=240)
    font = writer._add_object(DictionaryObject({
        NameObject("/Type"): NameObject("/Font"),
        NameObject("/Subtype"): NameObject("/Type1"),
        NameObject("/BaseFont"): NameObject("/Helvetica"),
    }))
    fonts = DictionaryObject({NameObject("/F1"): font})

    def form(data, resources):
        stream = DecodedStreamObject()
        stream.set_data(data.encode("ascii"))
        stream.update({
            NameObject("/Type"): NameObject("/XObject"),
            NameObject("/Subtype"): NameObject("/Form"),
            NameObject("/BBox"): ArrayObject([NumberObject(x) for x in (0, 0, 320, 240)]),
            NameObject("/Resources"): resources,
        })
        return writer._add_object(stream)

    local = state if explicit else ""
    leaf = form(f"{local} BT 1 0 0 1 35 130 Tm {position} (Form text) Tj ET",
                DictionaryObject({NameObject("/Font"): fonts}))
    target = leaf
    if nested:
        target = form("q /Leaf Do Q", DictionaryObject({
            NameObject("/XObject"): DictionaryObject({NameObject("/Leaf"): leaf}),
        }))
    page[NameObject("/Resources")] = DictionaryObject({
        NameObject("/Font"): fonts,
        NameObject("/XObject"): DictionaryObject({NameObject("/Form"): target}),
    })
    stream = DecodedStreamObject()
    # A conflicting saved state ensures q/Q restoration is exercised too.
    stream.set_data((f"q /F1 16 Tf {state} q 3 TL 1 Ts 90 Tz Q /Form Do Q").encode("ascii"))
    page[NameObject("/Contents")] = writer._add_object(stream)
    writer.write(path)


class Pdfium:
    def __init__(self, path):
        self.dll = c.CDLL(str(path.resolve()))
        signatures = {
            "FPDF_InitLibrary": (None, []),
            "FPDF_DestroyLibrary": (None, []),
            "FPDF_LoadDocument": (c.c_void_p, [c.c_char_p, c.c_char_p]),
            "FPDF_CloseDocument": (None, [c.c_void_p]),
            "FPDF_LoadPage": (c.c_void_p, [c.c_void_p, c.c_int]),
            "FPDF_ClosePage": (None, [c.c_void_p]),
            "FPDFText_LoadPage": (c.c_void_p, [c.c_void_p]),
            "FPDFText_ClosePage": (None, [c.c_void_p]),
            "FPDFText_CountChars": (c.c_int, [c.c_void_p]),
            "FPDFText_GetUnicode": (c.c_uint, [c.c_void_p, c.c_int]),
            "FPDFText_GetCharOrigin": (c.c_int, [c.c_void_p, c.c_int, c.POINTER(c.c_double), c.POINTER(c.c_double)]),
            "FPDFBitmap_Create": (c.c_void_p, [c.c_int, c.c_int, c.c_int]),
            "FPDFBitmap_Destroy": (None, [c.c_void_p]),
            "FPDFBitmap_FillRect": (c.c_int, [c.c_void_p, c.c_int, c.c_int, c.c_int, c.c_int, c.c_ulong]),
            "FPDFBitmap_GetBuffer": (c.c_void_p, [c.c_void_p]),
            "FPDFBitmap_GetStride": (c.c_int, [c.c_void_p]),
            "FPDF_RenderPageBitmap": (None, [c.c_void_p, c.c_void_p, c.c_int, c.c_int, c.c_int, c.c_int, c.c_int, c.c_int]),
        }
        for name, (result, args) in signatures.items():
            function = getattr(self.dll, name)
            function.restype = result
            function.argtypes = args
        self.dll.FPDF_InitLibrary()

    def render(self, path):
        dll = self.dll
        document = dll.FPDF_LoadDocument(str(path.resolve()).encode("utf-8"), None)
        if not document:
            raise RuntimeError(f"PDFium could not load {path}")
        page = text = bitmap = None
        try:
            page = dll.FPDF_LoadPage(document, 0)
            if not page:
                raise RuntimeError("PDFium could not load page 0")
            text = dll.FPDFText_LoadPage(page)
            if not text:
                raise RuntimeError("PDFium could not load text page")
            characters = []
            count = dll.FPDFText_CountChars(text)
            if count < 0:
                raise RuntimeError("PDFium could not count text characters")
            for index in range(count):
                x, y = c.c_double(), c.c_double()
                if not dll.FPDFText_GetCharOrigin(text, index, c.byref(x), c.byref(y)):
                    raise RuntimeError("PDFium could not retrieve character origin")
                characters.append({"unicode": dll.FPDFText_GetUnicode(text, index), "origin": [x.value, y.value]})
            bitmap = dll.FPDFBitmap_Create(640, 480, 0)
            if not bitmap:
                raise RuntimeError("PDFium bitmap allocation failed")
            if not dll.FPDFBitmap_FillRect(bitmap, 0, 0, 640, 480, 0xFFFFFFFF):
                raise RuntimeError("PDFium bitmap fill failed")
            dll.FPDF_RenderPageBitmap(bitmap, page, 0, 0, 640, 480, 0, 0)
            stride = dll.FPDFBitmap_GetStride(bitmap)
            buffer = dll.FPDFBitmap_GetBuffer(bitmap)
            if not buffer or stride < 640 * 4:
                raise RuntimeError("PDFium returned an invalid bitmap buffer")
            pixels = c.string_at(buffer, stride * 480)
            return Image.frombytes("RGB", (640, 480), pixels, "raw", "BGRX", stride), characters
        finally:
            if bitmap:
                dll.FPDFBitmap_Destroy(bitmap)
            if text:
                dll.FPDFText_ClosePage(text)
            if page:
                dll.FPDF_ClosePage(page)
            dll.FPDF_CloseDocument(document)

    def close(self):
        self.dll.FPDF_DestroyLibrary()


def mupdf(path):
    with fitz.open(path) as document:
        pixmap = document[0].get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False)
        return Image.frombytes("RGB", (pixmap.width, pixmap.height), pixmap.samples)


def has_ink(bitmap):
    return ImageChops.difference(bitmap, Image.new("RGB", bitmap.size, "white")).getbbox() is not None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dll", type=Path, default=Path("src-tauri/resources/pdfium/pdfium.dll"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expect-known-defects", action="store_true")
    args = parser.parse_args()
    if args.output.exists() and any(args.output.iterdir()):
        raise SystemExit("Choose a fresh output directory to preserve prior evidence")
    args.output.mkdir(parents=True, exist_ok=True)
    report_path = args.output / "results.json"
    report = {"dll": str(args.dll.resolve()), "dllSha256": digest(args.dll),
              "scriptSha256": digest(Path(__file__)), "mupdfVersion": fitz.VersionBind,
              "knownDefectMode": args.expect_known_defects, "cases": []}
    engine = Pdfium(args.dll)
    try:
        for name, (state, position, known_defect) in CASES.items():
            for nested in (False, True):
                label = f"{name}-{'nested' if nested else 'single'}"
                native, independent, origins, hashes = [], [], [], []
                for explicit in (False, True):
                    stem = f"{label}-{'explicit' if explicit else 'inherited'}"
                    pdf = args.output / f"{stem}.pdf"
                    fixture(pdf, state, position, explicit, nested)
                    bitmap, characters = engine.render(pdf)
                    bitmap.save(args.output / f"{stem}-pdfium.png")
                    reference = mupdf(pdf)
                    reference.save(args.output / f"{stem}-mupdf.png")
                    native.append(bitmap)
                    independent.append(reference)
                    origins.append(characters)
                    hashes.append(digest(pdf))
                native_difference = ImageChops.difference(*native).getbbox()
                independent_difference = ImageChops.difference(*independent).getbbox()
                expected_mismatch = args.expect_known_defects and known_defect
                content_valid = (
                    all(has_ink(bitmap) for bitmap in native + independent)
                    and all("".join(chr(char["unicode"]) for char in characters) == "Form text"
                            for characters in origins)
                )
                passed = content_valid and independent_difference is None and bool(native_difference) == expected_mismatch
                report["cases"].append({"name": label, "passed": passed,
                    "pdfSha256": hashes, "pdfiumDifference": native_difference,
                    "mupdfDifference": independent_difference, "contentValid": content_valid,
                    "pdfiumCharacters": origins})
                print(f"{label}: PDFium {'DIFFERS' if native_difference else 'equal'}, MuPDF {'DIFFERS' if independent_difference else 'equal'}", flush=True)
    finally:
        engine.close()
    report["passed"] = all(case["passed"] for case in report["cases"])
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    raise SystemExit(0 if report["passed"] else 1)


if __name__ == "__main__":
    main()
