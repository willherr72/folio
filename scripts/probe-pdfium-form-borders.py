"""Verify numeric widget borders through generation, interactive fill and save.

Synthetic input only. No JS, external callbacks, private files or installed DLL
mutation. Exact appearance geometry is checked independently with pypdf; MuPDF
also checks the top border band against a manually drawn reference.
"""
import argparse
import ctypes as c
import hashlib
import json
from pathlib import Path
import runpy

import fitz
from PIL import Image, ImageChops
from pypdf import PdfReader, PdfWriter
from pypdf.generic import (ArrayObject as A, BooleanObject as B, ContentStream,
                          DecodedStreamObject, DictionaryObject as D,
                          FloatObject as F, NameObject as N, NullObject, NumberObject as I,
                          TextStringObject as T)

HELPERS = runpy.run_path(str(Path(__file__).with_name('probe-pdfium-form-fonts.py')))
RECT = [20, 120, 300, 170]


def fixture(path, kind, width, storage, unit):
    writer = PdfWriter()
    page = writer.add_blank_page(320, 240)
    page[N('/UserUnit')] = F(unit)
    font = writer._add_object(D({N('/Type'): N('/Font'), N('/Subtype'): N('/Type1'),
                                N('/BaseFont'): N('/Helvetica')}))
    field = D({N('/Type'): N('/Annot'), N('/Subtype'): N('/Widget'),
               N('/FT'): N('/Tx' if kind == 'text' else '/Btn' if kind in ['checkbox', 'radio'] else '/Ch'), N('/T'): T('border'),
               N('/Rect'): A(map(I, RECT)), N('/F'): I(4),
               N('/MK'): D({N('/BC'): A([I(0)])}), N('/V'): T('')})
    if kind in ['combo', 'list']:
        field[N('/Ff')] = I((1 << 17) if kind == 'combo' else 0)
        field[N('/Opt')] = A([T('First'), T('Second')])
    elif kind in ['checkbox', 'radio']:
        field[N('/Ff')] = I((1 << 15) if kind == 'radio' else 0)
        field[N('/V')] = N('/Off')
        field[N('/AS')] = N('/Off')
        field['/MK'][N('/CA')] = T('l' if kind == 'radio' else '4')
    if storage == 'border':
        field[N('/Border')] = A([I(0), I(0), F(width)])
    elif storage == 'both':
        field[N('/Border')] = A([I(0), I(0), F(width)])
        field[N('/BS')] = D({N('/W'): F(7), N('/S'): N('/S')})
    elif storage != 'absent':
        field[N('/BS')] = D({N('/S'): N('/S')})
        if storage != 'default':
            value = B(True) if storage == 'boolean' else NullObject() if storage == 'null' else F(width)
            field['/BS'][N('/W')] = writer._add_object(value) if storage == 'indirect' else value
    ref = writer._add_object(field)
    page[N('/Annots')] = A([ref])
    writer._root_object[N('/AcroForm')] = D({N('/Fields'): A([ref]),
        N('/DA'): T('/F1 12 Tf 0 g'), N('/NeedAppearances'): B(True),
        N('/DR'): D({N('/Font'): D({N('/F1'): font})})})
    with path.open('xb') as output:
        writer.write(output)


def fill(engine, source, target, kind):
    dll = engine.dll
    signatures = {
        'FPDFPage_GetAnnot': (c.c_void_p, [c.c_void_p, c.c_int]),
        'FPDFPage_CloseAnnot': (None, [c.c_void_p]),
        'FORM_SetFocusedAnnot': (c.c_int, [c.c_void_p, c.c_void_p]),
        'FORM_SelectAllText': (c.c_int, [c.c_void_p, c.c_void_p]),
        'FORM_ReplaceSelection': (None, [c.c_void_p, c.c_void_p, c.c_void_p]),
        'FORM_SetIndexSelected': (c.c_int, [c.c_void_p, c.c_void_p, c.c_int, c.c_int]),
        'FORM_ForceToKillFocus': (c.c_int, [c.c_void_p]),
        'FORM_OnChar': (c.c_int, [c.c_void_p, c.c_void_p, c.c_int, c.c_int]),
    }
    for name, (result, args) in signatures.items():
        fn = getattr(dll, name); fn.restype = result; fn.argtypes = args
    # native_save has already established the pinned form/snapshot signatures.
    doc = dll.FPDF_LoadDocument(str(source).encode(), None)
    if not doc:
        raise RuntimeError('fill open failed')
    page = form = annot = None
    info = HELPERS['FormInfo'](); info.version = 2; info.xfa_disabled = 1
    chunks = bytearray()
    failed = False

    @HELPERS['WRITE']
    def write(_, data, size):
        nonlocal failed
        try:
            if failed or not data or size > 32 * 1024 * 1024 - len(chunks):
                failed = True
                return 0
            chunks.extend(c.string_at(data, size))
            return 1
        except Exception:
            failed = True
            return 0

    try:
        form = dll.FPDFDOC_InitFormFillEnvironment(doc, c.byref(info))
        if not form:
            raise RuntimeError('fill environment failed')
        page = dll.FPDF_LoadPage(doc, 0)
        if not page:
            raise RuntimeError('fill page failed')
        dll.FORM_OnAfterLoadPage(page, form)
        annot = dll.FPDFPage_GetAnnot(page, 0)
        if not annot or not dll.FORM_SetFocusedAnnot(form, annot):
            raise RuntimeError('focus failed')
        if kind == 'text':
            if not dll.FORM_SelectAllText(form, page):
                raise RuntimeError('select text failed')
            text = c.create_string_buffer('Edited answer\0'.encode('utf-16-le'))
            dll.FORM_ReplaceSelection(form, page, text)
        elif kind in ['checkbox', 'radio']:
            if not dll.FORM_OnChar(form, page, 32, 0):
                raise RuntimeError('toggle failed')
        elif not dll.FORM_SetIndexSelected(form, page, 1, 1):
            raise RuntimeError('select option failed')
        if not dll.FORM_ForceToKillFocus(form):
            raise RuntimeError('commit failed')
        writer = HELPERS['FileWrite'](1, write)
        if not dll.FPDF_SaveAsCopy(doc, c.byref(writer), 0) or failed:
            raise RuntimeError('bounded save failed')
        with target.open('xb') as output:
            output.write(chunks)
    finally:
        if annot:
            dll.FPDFPage_CloseAnnot(annot)
        if page:
            if form:
                dll.FORM_OnBeforeClosePage(page, form)
            dll.FPDF_ClosePage(page)
        if form:
            dll.FPDFDOC_ExitFormFillEnvironment(form)
        dll.FPDF_CloseDocument(doc)


def inspect(path, width, value=None, kind='text'):
    reader = PdfReader(path)
    field = reader.pages[0]['/Annots'][0].get_object()
    if value is not None:
        assert field.get('/V') == value, ('answer changed', field.get('/V'), value)
    if kind in ['checkbox', 'radio']:
        evidence = {}
        for mode in ['/N', '/D']:
            states = field['/AP'][mode]
            assert set(states) == {'/Off', '/Yes'}, ('appearance states changed', states)
            for state in states:
                ops = ContentStream(states[state], reader).operations
                if kind == 'radio':
                    widths = [float(args[0]) for args, op in ops if op == b'w']
                    assert widths == [] if width == 0 else widths and all(abs(n-width) < .0001 for n in widths), ('circle width changed', widths, width)
                    if width > 0:
                        move = None
                        strokes = []
                        for args, op in ops:
                            if op == b'm':
                                move = args
                            elif op == b'S':
                                strokes.append(move)
                        assert strokes and all(move and abs(float(move[0])-(116+width/2)) < .0001
                                               for move in strokes), ('circle placement changed', strokes)
                    evidence[mode+state] = widths
                else:
                    evidence[mode+state] = inspect_rectangles(ops, width)
        return evidence
    return inspect_rectangles(ContentStream(field['/AP']['/N'], reader).operations, width)


def inspect_rectangles(operations, width):
    rectangles = []
    for operands, operator in operations:
        if operator == b're':
            rectangles.append([float(n) for n in operands])
        if operator == b'f*':
            break
        if operator in [b'f', b'F', b'S', b'n', b'B', b'B*', b'b', b'b*', b's']:
            rectangles = []
    expected = [[0, 0, 280, 50], [width, width, 280 - 2*width, 50 - 2*width]]
    if width > 0:
        assert len(rectangles) >= 2, ('border missing', rectangles)
        assert all(abs(a-b) < 0.0001 for actual, wanted in zip(rectangles[:2], expected)
                   for a, b in zip(actual, wanted)), ('border geometry changed', rectangles[:2], expected)
    else:
        assert not (len(rectangles) >= 2 and rectangles[0] == expected[0]
                    and rectangles[1][0] > 0), 'zero width gained an outer border'
    return rectangles[:2]


def raster_check(path, width):
    """Compare a border band away from all text/dropdown controls at 4 pixels/unit."""
    writer = PdfWriter(clone_from=path)
    field = writer.pages[0]['/Annots'][0].get_object()
    appearance = DecodedStreamObject()
    appearance.set_data((f'0 g 0 0 280 50 re {width} {width} {280-2*width} {50-2*width} re f*'
                         if width > 0 else '').encode('ascii'))
    appearance.update({N('/Type'): N('/XObject'), N('/Subtype'): N('/Form'),
                       N('/BBox'): A(map(I, [0, 0, 280, 50]))})
    field[N('/AP')] = D({N('/N'): writer._add_object(appearance)})
    reference = path.with_name(path.stem + '-reference.pdf')
    with reference.open('xb') as output:
        writer.write(output)
    images = []
    for current in [path, reference]:
        with fitz.open(current) as doc:
            unit = float(PdfReader(current).pages[0].get('/UserUnit', 1))
            pix = doc[0].get_pixmap(matrix=fitz.Matrix(4/unit, 4/unit), alpha=False)
            image = Image.frombytes('RGB', [pix.width, pix.height], pix.samples)
            # Page top = 240-170 = 70. Include outside border and enough interior
            # to detect widths up to 2, with no text/choice widgets near this band.
            images.append(image.crop((120, 276, 1000, 292)))
    difference = ImageChops.difference(*images).getbbox()
    assert difference is None, ('independent border pixels differ', difference)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dll', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(); args.output.mkdir(parents=True, exist_ok=False)
    engine = HELPERS['HELPERS']['Pdfium'](args.dll)
    cases = []
    try:
        variants = [('bs', width, 1) for width in [0, .5, 1, 1.5, 2]]
        variants += [('border', .5, 1), ('both', 1.5, 1), ('default', 1, 1),
                     ('bs', 1/1.7, 1.7), ('absent', 1, 1), ('indirect', .5, 1),
                     ('boolean', 1, 1), ('null', 0, 1)]
        for kind in ['text', 'combo', 'list', 'checkbox', 'radio']:
            for storage, width, unit in variants:
                name = f'{kind}-{storage}-{width:.5f}-{unit}'
                source = args.output / f'{name}-source.pdf'
                generated = args.output / f'{name}-generated.pdf'
                filled = args.output / f'{name}-filled.pdf'
                fixture(source, kind, width, storage, unit)
                HELPERS['native_save'](engine, source, generated)
                fill(engine, generated, filled, kind)
                for phase, path in [('generated', generated), ('filled', filled)]:
                    try:
                        value = ('Edited answer' if kind == 'text' else '/Yes' if kind in ['checkbox', 'radio'] else 'Second') if phase == 'filled' else None
                        geometry = inspect(path, width, value, kind)
                        if kind != 'radio':
                            raster_check(path, width)
                        result = dict(case=name, phase=phase, passed=True, geometry=geometry)
                    except (AssertionError, KeyError) as error:
                        result = dict(case=name, phase=phase, passed=False, error=str(error))
                    cases.append(result)
                    print(name, phase, result['passed'], result.get('error', ''), flush=True)
    finally:
        engine.close()
    report = dict(passed=all(row['passed'] for row in cases), cases=cases,
                  dllSha256=hashlib.sha256(args.dll.read_bytes()).hexdigest(),
                  scriptSha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  limits=['Solid rectangular text/choice/checkbox and circular radio borders; no custom artwork proof',
                          'Saved appearance and value checks; not an interactive screenshot or desktop acceptance'])
    (args.output/'results.json').write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
    raise SystemExit(0 if report['passed'] else 1)


if __name__ == '__main__':
    main()
