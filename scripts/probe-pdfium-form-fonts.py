"""Verify original embedded form fonts and unchanged missing-glyph fallback."""
import argparse
import ctypes as c
import hashlib
import json
from pathlib import Path
import runpy

from pypdf import PdfReader, PdfWriter
from pypdf.generic import (ArrayObject as A, BooleanObject as B, ContentStream,
                          DecodedStreamObject, DictionaryObject as D,
                          NameObject as N, NumberObject as I, TextStringObject as T)

ROOT = Path(__file__).resolve().parent.parent
FIXTURE = ROOT / 'tests/fixtures/pdfium-form-fonts/greek-source.pdf'
HELPERS = runpy.run_path(str(Path(__file__).with_name('probe-pdfium-form-inheritance.py')))


class FormInfo(c.Structure):
    _fields_ = [('version', c.c_int), ('callbacks', c.c_void_p * 16),
                ('xfa_disabled', c.c_int), ('extensions', c.c_void_p * 17)]


WRITE = c.CFUNCTYPE(c.c_int, c.c_void_p, c.c_void_p, c.c_ulong)


class FileWrite(c.Structure):
    _fields_ = [('version', c.c_int), ('write', WRITE)]


def native_save(engine, source, target):
    dll = engine.dll
    signatures = {
        'FPDFDOC_InitFormFillEnvironment': (c.c_void_p, [c.c_void_p, c.c_void_p]),
        'FPDFDOC_ExitFormFillEnvironment': (None, [c.c_void_p]),
        'FORM_OnAfterLoadPage': (None, [c.c_void_p, c.c_void_p]),
        'FORM_OnBeforeClosePage': (None, [c.c_void_p, c.c_void_p]),
        'FPDF_SaveAsCopy': (c.c_int, [c.c_void_p, c.c_void_p, c.c_ulong]),
    }
    for name, (result, args) in signatures.items():
        fn = getattr(dll, name); fn.restype = result; fn.argtypes = args
    assert c.sizeof(c.c_void_p) == 8 and c.sizeof(FormInfo) == 280
    assert FormInfo.xfa_disabled.offset == 136
    doc = dll.FPDF_LoadDocument(str(source).encode(), None)
    if not doc:
        raise RuntimeError('native open failed')
    form = page = None
    chunks = bytearray()
    failed = False

    @WRITE
    def write(_, data, size):
        nonlocal failed
        if failed or not data or size > 32 * 1024 * 1024 - len(chunks):
            failed = True
            return 0
        try:
            chunks.extend(c.string_at(data, size))
            return 1
        except Exception:
            failed = True
            return 0

    try:
        info = FormInfo(); info.version = 2; info.xfa_disabled = 1
        form = dll.FPDFDOC_InitFormFillEnvironment(doc, c.byref(info))
        if not form:
            raise RuntimeError('native form initialization failed')
        page = dll.FPDF_LoadPage(doc, 0)
        if not page:
            raise RuntimeError('native page load failed')
        dll.FORM_OnAfterLoadPage(page, form)
        writer = FileWrite(1, write)
        if not dll.FPDF_SaveAsCopy(doc, c.byref(writer), 0) or failed:
            raise RuntimeError('bounded native snapshot failed')
        with target.open('xb') as output:
            output.write(chunks)
    finally:
        if page:
            if form:
                dll.FORM_OnBeforeClosePage(page, form)
            dll.FPDF_ClosePage(page)
        if form:
            dll.FPDFDOC_ExitFormFillEnvironment(form)
        dll.FPDF_CloseDocument(doc)


def font_of(reader):
    return reader.trailer['/Root']['/AcroForm']['/DR']['/Font']['/F1']


def code_map(font):
    raw = font['/ToUnicode'].get_data().split(b'beginbfchar', 1)[1].split(b'endbfchar', 1)[0]
    import re
    return {bytes.fromhex(a.decode()): bytes.fromhex(b.decode()).decode('utf-16-be')
            for a, b in re.findall(rb'<([0-9A-Fa-f]+)>\s*<([0-9A-Fa-f]+)>', raw)}


def fixture(path, kind, text, variant):
    writer = PdfWriter(clone_from=FIXTURE)
    form = writer._root_object['/AcroForm']
    field = form['/Fields'][0].get_object()
    for key in ['/AP', '/V', '/I', '/Opt', '/DV']:
        field.pop(key, None)
    field[N('/FT')] = N('/Tx' if kind == 'text' else '/Ch')
    field[N('/Ff')] = I(1 << 17 if kind == 'combo' else 0)
    field[N('/DA')] = T('/F1 12 Tf 0 g')
    if kind == 'text':
        field[N('/V')] = T(text)
    else:
        field[N('/Opt')] = A([A([T('key'), T(text)])])
        field[N('/V')] = T('key')
        field[N('/I')] = A([I(0)])
        field[N('/TI')] = I(0)
    font = form['/DR']['/Font']['/F1']
    if variant == 'nonembedded':
        form['/DR']['/Font'][N('/F1')] = writer._add_object(D({N('/Type'): N('/Font'),
            N('/Subtype'): N('/Type1'), N('/BaseFont'): N('/Helvetica')}))
    elif variant in ['missing-glyph', 'out-of-range-glyph']:
        mapping = code_map(font)
        omega = next(int.from_bytes(code, 'big') for code, char in mapping.items() if char == 'Ω')
        count = max(int.from_bytes(code, 'big') for code in mapping) + 1
        data = bytearray(b''.join(i.to_bytes(2, 'big') for i in range(count)))
        data[omega*2:omega*2+2] = (0 if variant == 'missing-glyph' else 65535).to_bytes(2, 'big')
        stream = DecodedStreamObject(); stream.set_data(bytes(data))
        font['/DescendantFonts'][0].get_object()[N('/CIDToGIDMap')] = writer._add_object(stream)
    form[N('/NeedAppearances')] = B(True)
    with path.open('xb') as output:
        writer.write(output)


def inspect(source, output, text, preserve):
    before, after = PdfReader(source), PdfReader(output)
    appearance = after.pages[0]['/Annots'][0].get_object()['/AP']['/N']
    fonts = appearance['/Resources']['/Font']
    used = []
    chunks = []
    current = None
    for operands, operator in ContentStream(appearance, after).operations:
        if operator == b'Tf':
            current = str(operands[0])
        elif operator == b'Tj':
            used.append(current)
            value = operands[0]
            chunks.append(value.original_bytes if hasattr(value, 'original_bytes') else bytes(value))
    if preserve:
        assert used and set(used) == {'/F1'}, ('original font substituted', used)
        old_font, new_font = font_of(before), fonts['/F1']
        program = lambda f: f['/DescendantFonts'][0].get_object()['/FontDescriptor']['/FontFile2'].get_data()
        assert program(old_font) == program(new_font), 'embedded program changed'
        mapping = code_map(old_font)
        decoded = ''
        for chunk in chunks:
            assert len(chunk) % 2 == 0
            decoded += ''.join(mapping[chunk[i:i+2]] for i in range(0, len(chunk), 2))
        assert decoded == text, ('native text changed', decoded, text)
    else:
        assert used and any(name != '/F1' for name in used), ('missing glyph must retain fallback', used)
    return {'usedFonts': used, 'preservedOriginal': preserve}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dll', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(); args.output.mkdir(parents=True, exist_ok=False)
    engine = HELPERS['Pdfium'](args.dll)
    results = []
    try:
        for kind in ['combo', 'list', 'text']:
            for label, text, variant in [('latin','A','original'), ('greek','Ω','original'),
                    ('mixed','AΩé','original'), ('missing','Ω','missing-glyph'),
                    ('out-of-range','Ω','out-of-range-glyph'), ('nonembedded','Ω','nonembedded')]:
                name = f'{kind}-{label}'; source = args.output / f'{name}-source.pdf'
                output = args.output / f'{name}-native.pdf'
                fixture(source, kind, text, variant)
                native_save(engine, source, output)
                try:
                    evidence = inspect(source, output, text, variant == 'original')
                    result = {'case': name, 'passed': True, **evidence}
                except (AssertionError, KeyError) as error:
                    result = {'case': name, 'passed': False, 'error': str(error)}
                results.append(result)
                print(name, result['passed'], result.get('error', ''), flush=True)
    finally:
        engine.close()
    report = {'passed': all(r['passed'] for r in results), 'cases': results,
        'dllSha256': hashlib.sha256(args.dll.read_bytes()).hexdigest(),
        'fixtureSha256': hashlib.sha256(FIXTURE.read_bytes()).hexdigest(),
        'scriptSha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'limits': ['BMP Unicode codes and embedded CID TrueType; not supplementary/shaping/layout proof',
                   'Synthetic trusted probes; no public choice-editor or release acceptance']}
    (args.output/'results.json').write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
    raise SystemExit(0 if report['passed'] else 1)


if __name__ == '__main__':
    main()
