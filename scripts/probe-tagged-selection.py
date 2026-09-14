"""Experimental tagged-PDF and selection-copy evidence; never release acceptance.

Keep page extraction and selection-copy strings separate and completely unmodified.
The strict production-area verifier is unchanged. MuPDF APIs are engine evidence,
not a substitute for interactive Foxit/Acrobat clipboard acceptance.
"""
import argparse
import ctypes as c
import hashlib
import importlib.util
import json
from pathlib import Path

import fitz
import pypdf
from pypdf.generic import (
    ArrayObject as A, BooleanObject, DictionaryObject as D, NameObject as N,
    NumberObject as I, TextStringObject as T,
)

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('separator', ROOT / 'scripts/probe-text-separators.py')
separator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(separator)
MODES = ['tags-only', 'structure-actualtext', 'marked-actualtext', 'both-actualtext']
CASES = [('single', 'A B', False), ('spaces', 'A  B', False),
         ('soft', 'A B C D', True), ('soft-spaces', 'A  B C D', True),
         ('hard', 'A\nB', False)]


def tag_pdf(source, target, logical, mode):
    if mode not in MODES:
        raise ValueError('Unknown tagging strategy')
    writer = pypdf.PdfWriter(clone_from=source)
    page = writer.pages[0]
    content = page.get_contents().get_data()
    ink, semantic = content.split(b'BT ', 1)
    properties = b'/MCID 0'
    if mode in ['marked-actualtext', 'both-actualtext']:
        properties += b' /ActualText <FEFF' + logical.encode('utf-16-be').hex().encode() + b'>'
    page[N('/Contents')] = writer._add_object(separator.reader.stream(
        b'/Artifact BMC\n' + ink + b'\nEMC\n/Span << ' + properties +
        b' >> BDC\nBT ' + semantic + b'\nEMC\n'))
    tree = D({N('/Type'): N('/StructTreeRoot')})
    tree_ref = writer._add_object(tree)
    document = D({N('/Type'): N('/StructElem'), N('/S'): N('/Document'), N('/P'): tree_ref})
    document_ref = writer._add_object(document)
    paragraph = D({N('/Type'): N('/StructElem'), N('/S'): N('/P'), N('/P'): document_ref})
    paragraph_ref = writer._add_object(paragraph)
    span = D({N('/Type'): N('/StructElem'), N('/S'): N('/Span'), N('/P'): paragraph_ref,
              N('/Pg'): page.indirect_reference, N('/K'): I(0)})
    if mode in ['structure-actualtext', 'both-actualtext']:
        span[N('/ActualText')] = T(logical)
    span_ref = writer._add_object(span)
    tree[N('/K')] = document_ref
    document[N('/K')] = paragraph_ref
    paragraph[N('/K')] = span_ref
    tree[N('/ParentTree')] = writer._add_object(D({N('/Nums'): A([I(0), A([span_ref])])}))
    tree[N('/ParentTreeNextKey')] = I(1)
    page[N('/StructParents')] = I(0)
    writer.root_object[N('/StructTreeRoot')] = tree_ref
    writer.root_object[N('/MarkInfo')] = D({N('/Marked'): BooleanObject(True)})
    writer.root_object[N('/Lang')] = T('en-US')
    writer.write(target)


def mupdf_selection(path):
    with fitz.open(path) as pdf:
        page = pdf[0]
        # MuPDF text coordinates are unrotated page coordinates even for /Rotate.
        start = fitz.mupdf.FzPoint(0, 0)
        end = fitz.mupdf.FzPoint(page.cropbox.width, page.cropbox.height)
        textpage = page.get_textpage()
        result = {'pageText': page.get_text(), 'selectionStart': [start.x, start.y],
                  'selectionEnd': [end.x, end.y]}
        result['characters'] = [char for block in page.get_text('rawdict')['blocks']
                                for line in block.get('lines', [])
                                for span in line['spans'] for char in span['chars']]
        for crlf, name in [(0, 'selectionLf'), (1, 'selectionCrlf')]:
            result[name] = fitz.mupdf.fz_copy_selection(textpage.this, start, end, crlf)
        # Record structure-aware mode separately; do not choose whichever passes.
        structured = page.get_textpage(flags=fitz.TEXTFLAGS_TEXT | fitz.TEXT_COLLECT_STRUCTURE)
        result['structuredSelectionLf'] = fitz.mupdf.fz_copy_selection(structured.this, start, end, 0)
        pix = page.get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False)
        result['raster'] = [pix.width, pix.height, hashlib.sha256(pix.samples).hexdigest()]
        return result


def pdfium_text(dll, path):
    doc = page = textpage = None
    try:
        doc = dll.FPDF_LoadDocument(str(path).encode(), None)
        if not doc:
            raise RuntimeError('PDFium document load failed')
        page = dll.FPDF_LoadPage(doc, 0)
        if not page:
            raise RuntimeError('PDFium page load failed')
        textpage = dll.FPDFText_LoadPage(page)
        if not textpage:
            raise RuntimeError('PDFium text load failed')
        count = dll.FPDFText_CountChars(textpage)
        buffer = (c.c_ushort * (count + 1))()
        length = dll.FPDFText_GetText(textpage, 0, count, buffer)
        if length < 1 or length > count + 1:
            raise RuntimeError('PDFium text extraction failed')
        return bytes(buffer)[:(length - 1) * 2].decode('utf-16-le')
    finally:
        if textpage:
            dll.FPDFText_ClosePage(textpage)
        if page:
            dll.FPDF_ClosePage(page)
        if doc:
            dll.FPDF_CloseDocument(doc)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    dllpath = ROOT / 'src-tauri/resources/pdfium/pdfium.dll'
    dll = separator.reader.native_reader(dllpath)
    dll.FPDFText_GetText.argtypes = [c.c_void_p, c.c_int, c.c_int, c.POINTER(c.c_ushort)]
    dll.FPDFText_GetText.restype = c.c_int
    dll.FPDF_InitLibrary()
    rows = []
    try:
        for name, logical, wrap in CASES:
            for rotation in [0, 90, 180, 270]:
                source = args.output / f'{name}-{rotation}-untagged.pdf'
                cells = separator.write_case(source, logical, wrap, 'scalar-cells', rotation)
                reference = mupdf_selection(source)['raster']
                for mode in ['untagged'] + MODES:
                    path = source if mode == 'untagged' else args.output / f'{name}-{rotation}-{mode}.pdf'
                    if mode != 'untagged':
                        tag_pdf(source, path, logical, mode)
                    mu = mupdf_selection(path)
                    native = separator.reader.inspect(dll, path)
                    texts = {'pdfiumPageText': pdfium_text(dll, path),
                             'mupdfPageText': mu['pageText'],
                             'mupdfSelectionLf': mu['selectionLf'],
                             'mupdfSelectionCrlf': mu['selectionCrlf'],
                             'mupdfStructuredSelectionLf': mu['structuredSelectionLf'],
                             'pypdfPageText': pypdf.PdfReader(path).pages[0].extract_text()}
                    rows.append({'case': name, 'rotation': rotation, 'mode': mode,
                                 'expected': logical, 'texts': texts,
                                 'exact': {key: value == logical for key, value in texts.items()},
                                 'pdfiumCellBoxesExact': separator.exact_boxes(native['characters'], cells),
                                 'pdfiumCharacters': native['characters'],
                                 'mupdfCharacters': mu['characters'],
                                 'mupdfOriginsExact': len(mu['characters']) == len(cells) and all(
                                     char['c'] == cell['text'] and
                                     max(abs(a - b) for a, b in zip(char['origin'], [cell['x'], 180 - cell['y']])) < .02
                                     for char, cell in zip(mu['characters'], cells)),
                                 'selectionPoints': [mu['selectionStart'], mu['selectionEnd']],
                                 'unchangedInk': mu['raster'] == reference,
                                 'raster': mu['raster'], 'pdf': path.name,
                                 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
    finally:
        dll.FPDF_DestroyLibrary()
    if len(rows) != 100 or not all(row['unchangedInk'] for row in rows):
        raise RuntimeError('Incomplete matrix or changed appearance')
    report = {'description': __doc__, 'mupdf': fitz.VersionBind, 'pypdf': pypdf.__version__,
              'pdfiumSha256': hashlib.sha256(dllpath.read_bytes()).hexdigest(),
              'releaseAcceptance': False, 'cases': rows}
    (args.output / 'results.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    for mode in ['untagged'] + MODES:
        group = [row for row in rows if row['mode'] == mode]
        print(mode, {key: sum(row['exact'][key] for row in group) for key in group[0]['exact']},
              'cellBoxes', sum(row['pdfiumCellBoxesExact'] for row in group), 'of', len(group))


if __name__ == '__main__':
    main()
