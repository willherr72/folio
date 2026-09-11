"""Research PDFium ordering by changing only copied Type3 CMap space mappings.

This does not reshape the source or establish geometry for the substituted text.
It isolates the effect of Unicode separator classes on PDFium character order.
Original PDFs remain untouched. Requires the pre-refusal repeated-Arabic PDF.
"""
import argparse
import ctypes as c
import hashlib
import json
from pathlib import Path
from pypdf import PdfWriter

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--input', type=Path, default=root/'tests/fixtures/shaped-text/semantic-negative/rtl-word-order.pdf')
parser.add_argument('--output', type=Path, default=root/'artifacts/shaped-text/rtl-separator-probe')
args = parser.parse_args()
source, out = args.input, args.output
out.mkdir(parents=True, exist_ok=True)
dll_path = root / 'src-tauri/resources/pdfium/pdfium.dll'
dll = c.CDLL(str(dll_path))
for name, args, result in [
    ('FPDF_InitLibrary', [], None), ('FPDF_DestroyLibrary', [], None),
    ('FPDF_LoadDocument', [c.c_char_p, c.c_char_p], c.c_void_p),
    ('FPDF_CloseDocument', [c.c_void_p], None),
    ('FPDF_LoadPage', [c.c_void_p, c.c_int], c.c_void_p),
    ('FPDF_ClosePage', [c.c_void_p], None),
    ('FPDFText_LoadPage', [c.c_void_p], c.c_void_p),
    ('FPDFText_ClosePage', [c.c_void_p], None),
    ('FPDFText_CountChars', [c.c_void_p], c.c_int),
    ('FPDFText_GetUnicode', [c.c_void_p, c.c_int], c.c_uint),
    ('FPDFText_GetCharBox', [c.c_void_p, c.c_int] + [c.POINTER(c.c_double)]*4, c.c_int),
]:
    function = getattr(dll, name)
    function.argtypes = args
    function.restype = result
def inspect_copy(label, scalar):
    """All native handles are released even if a reader call fails."""
    writer = PdfWriter(clone_from=source)
    if len(writer.pages) != 1:
        raise ValueError('Probe requires exactly one source page')
    font = writer.pages[0]['/Resources']['/Font']['/S']
    cmap = font['/ToUnicode']
    if b'<0020>' not in cmap.get_data():
        raise ValueError('Probe input must contain a semantic space mapping')
    cmap.set_data(cmap.get_data().replace(b'<0020>', ('<%04X>' % ord(scalar)).encode()))
    path = out / (label + '.pdf')
    if path.resolve() == source.resolve():
        raise ValueError('Probe output must not overwrite its input')
    writer.write(path)
    document = page = text = None
    try:
        document = dll.FPDF_LoadDocument(str(path).encode('utf-8'), None)
        if not document:
            raise RuntimeError(f'PDFium could not open probe PDF: {path}')
        page = dll.FPDF_LoadPage(document, 0)
        if not page:
            raise RuntimeError(f'PDFium could not load probe page: {path}')
        text = dll.FPDFText_LoadPage(page)
        if not text:
            raise RuntimeError(f'PDFium could not load probe text: {path}')
        count = dll.FPDFText_CountChars(text)
        if count <= 0:
            raise RuntimeError(f'PDFium returned no probe characters: {path}')
        chars = []
        for index in range(count):
            left, right, bottom, top = (c.c_double() for _ in range(4))
            if not dll.FPDFText_GetCharBox(text, index, c.byref(left), c.byref(right), c.byref(bottom), c.byref(top)):
                raise RuntimeError(f'PDFium returned no box for probe character {index}: {path}')
            chars.append(dict(text=chr(dll.FPDFText_GetUnicode(text,index)), x=left.value))
        return dict(separator=label, chars=chars[:8], last=chars[-5:], count=len(chars))
    finally:
        if text:
            dll.FPDFText_ClosePage(text)
        if page:
            dll.FPDF_ClosePage(page)
        if document:
            dll.FPDF_CloseDocument(document)


dll.FPDF_InitLibrary()
try:
    rows = [inspect_copy(label, scalar) for label, scalar in [
                      ('space', ' '), ('nbsp', '\u00a0'), ('arabic-comma', '\u060c'),
                      ('comma', ','), ('hyphen', '-'), ('arabic-semicolon', '\u061b'),
                      ('arabic-letter', '\u0633'), ('zwnj', '\u200c'), ('arabic-question', '\u061f')]]
finally:
    dll.FPDF_DestroyLibrary()
report = dict(source_pdf_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
              pdfium_sha256=hashlib.sha256(dll_path.read_bytes()).hexdigest(),
              scope='CMap-only substitutions; no reshaping or substituted-glyph geometry claim.', probes=rows)
(out/'results.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
print(json.dumps([dict(separator=row['separator'], first_x=row['chars'][0]['x'], count=row['count']) for row in rows]))
