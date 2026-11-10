"""Check generated separators across equivalent nested and page text.

Only synthetic PDFs are created. --baseline-dll additionally requires unchanged
pixels/physical glyphs, and unchanged complete extraction for excluded controls.
Outputs are exclusive and failures retain all per-case evidence.
"""
import argparse
import ctypes as c
from decimal import Decimal
import hashlib
import json
from pathlib import Path
import runpy

from pypdf import PdfWriter
from pypdf.generic import ArrayObject as A, DictionaryObject as D, NameObject as N
from pypdf.generic import NumberObject as I, FloatObject as F, DecodedStreamObject

HELPERS = runpy.run_path(str(Path(__file__).with_name('probe-pdfium-form-inheritance.py')))
IDENTITY = [1, 0, 0, 1, 0, 0]


def number(value):
    return format(Decimal(str(value)), 'f')


class ProbeReal(F):
    def myrepr(self):
        # Huge integral coordinates must be PDF reals, not overflowing integers.
        value = number(float(self))
        return value if '.' in value else value + '.0'


def fixture(path, case, flat):
    writer = PdfWriter()
    page = writer.add_blank_page(500, 500)
    page[N('/Rotate')] = I(case.get('rotation', 0))
    page[N('/UserUnit')] = F(case.get('unit', 1))
    font = writer._add_object(D({N('/Type'): N('/Font'), N('/Subtype'): N('/Type1'),
                                N('/BaseFont'): N('/Helvetica')}))
    fonts = D({N('/Font'): D({N('/F1'): font})})

    def stream(data, fields=None):
        value = DecodedStreamObject()
        value.set_data(data.encode('ascii'))
        if fields:
            value.update(fields)
        return writer._add_object(value)

    text_matrix = case.get('textMatrix', [1, 0, 0, 1, 5, 15])
    text_matrices = case.get('textMatrices', [text_matrix, text_matrix])
    literal = case.get('text', 'Alpha')
    texts = ['BT /F1 16 Tf ' + ' '.join(map(number, matrix)) + f' Tm ({literal}) Tj ET' for matrix in text_matrices]
    fields = D({N('/Type'): N('/XObject'), N('/Subtype'): N('/Form'),
                N('/BBox'): A(map(I, [-1000, -1000, 1000, 1000]))})
    targets = []
    for index, text in enumerate(texts):
        if index and text == texts[0]:
            targets.append(targets[0])
            continue
        target_fields = D({**fields, N('/Resources'): fonts})
        if 'bounds' in case:
            target_fields[N('/BBox')] = A(map(ProbeReal, case['bounds'][index]))
        target = stream(text, target_fields)
        for _ in range(case.get('depth', 1) - 1):
            target = stream('/Inner Do', D({**fields, N('/Resources'): D({
                N('/XObject'): D({N('/Inner'): target})})}))
        targets.append(target)
    page[N('/Resources')] = D({**fonts, N('/XObject'): D({N('/Leaf'): targets[0],
                                                       N('/Alias'): targets[1]})})
    body = []
    for index, matrix in enumerate(case['matrices']):
        nested = case.get('nested', [True, True])[index] and not flat
        paint = ('/Alias Do' if index else '/Leaf Do') if nested else texts[index]
        body.append('q ' + ' '.join(map(number, matrix)) + ' cm ' + paint + ' Q')
    page[N('/Contents')] = stream(' '.join(body))
    with path.open('xb') as output:
        writer.write(output)


def cases():
    result = []
    for sx, sy in [(1, 1), (.8, .8), (1.2, .6)]:
        for line in [True, False]:
            for rotation, unit in [(0, 1), (90, 1.7), (180, 1), (270, 1.7)]:
                result.append({'name': f'axis-{len(result)}',
                    'matrices': [[1, 0, 0, 1, 40, 300], [sx, 0, 0, sy, 160, 110 if line else 315 - 15 * sy]],
                    'rotation': rotation, 'unit': unit, 'expected': 'Alpha\r\nAlpha' if line else 'Alpha Alpha'})
    for nesting in [[False, True], [True, False], [True, True]]:
        for depth in [1, 3]:
            for line in [True, False]:
                result.append({'name': f'mixed-{len(result)}', 'nested': nesting, 'depth': depth,
                    'matrices': [[.5, 0, 0, 2, 40, 280], [2, 0, 0, .5, 170, 100 if line else 302.5]],
                    'expected': 'Alpha\r\nAlpha' if line else 'Alpha Alpha'})
    controls = [
        ('same-transform', [1, 0, 0, 1, 40, 300], [1, 0, 0, 1, 40, 300]),
        ('mirror', [-1, 0, 0, 1, 200, 300], [-1, 0, 0, 1, 300, 110]),
        ('rotation', [0, 1, -1, 0, 100, 100], [0, 1, -1, 0, 300, 100]),
        ('shear', [1, .2, .3, 1, 40, 300], [1, .2, .3, 1, 160, 110]),
        ('singular-x', [0, 0, 0, 1, 40, 300], [0, 0, 0, 1, 160, 110]),
        ('singular-y', [1, 0, 0, 0, 40, 300], [1, 0, 0, 0, 160, 110]),
        ('tiny', [1e-25, 0, 0, 1e-25, 40, 300], [1, 0, 0, 1, 160, 110]),
        ('huge', [1e25, 0, 0, 1e25, 40, 300], [1, 0, 0, 1, 160, 110]),
        ('overflow-input', [1e100, 0, 0, 1e100, 40, 300], [1, 0, 0, 1, 160, 110]),
        ('underflow-input', [1e-100, 0, 0, 1e-100, 40, 300], [1, 0, 0, 1, 160, 110]),
        ('mixed-shear', [1, .2, .3, 1, 40, 300], [1, 0, 0, 1, 160, 110]),
        ('mixed-rotation', [1, 0, 0, 1, 40, 300], [0, 1, -1, 0, 160, 110]),
    ]
    for name, first, second in controls:
        result.append({'name': name, 'matrices': [first, second], 'unchanged': True})
    result.append({'name': 'vertical-local', 'textMatrix': [0, 1, -1, 0, 5, 15],
                   'matrices': [[1, 0, 0, 1, 100, 100], [1, 0, 0, 1, 300, 100]], 'unchanged': True})
    result.append({'name': 'mixed-orientation', 'textMatrices': [[0, 1, -1, 0, 5, 15], [1, 0, 0, 1, 5, 15]],
                   'matrices': [[1, 0, 0, 1, 100, 100], [1, 0, 0, 1, 300, 100]], 'unchanged': True})
    # Compensated text stays physically ordinary while relative Form axes underflow.
    # Invocation cm is intentional: Form /Matrix is folded into local text objects.
    scale = 2**100
    real = lambda value: str(value) + '.0'
    small, large = 2**-90, 2**110
    # Its literal oracle deliberately rejects v6's lost line break; the installed
    # comparison DLL can be that buggy intermediate version. Still require exact
    # physical glyph/pixel preservation, rather than its incorrect generated text.
    result.append({'name': 'relative-y-underflow', 'text': 'gyp',
                   'matrices': [[1, 0, 0, real(scale), 0, 384], [1, 0, 0, 1/scale, 0, 384]],
                   'textMatrices': [[1, 0, 0, 1/scale, 40, 0], [1, 0, 0, real(scale), 160, real(-128*scale)]],
                   'bounds': [[-500, -small, 500, small], [-500, -large, 500, large]],
                   'expected': 'gyp\r\ngyp'})
    result.append({'name': 'relative-x-underflow', 'text': 'gyp', 'unchanged': True,
                   'matrices': [[real(scale), 0, 0, 1, 0, 384], [1/scale, 0, 0, 1, 0, 256]],
                   'textMatrices': [[1/scale, 0, 0, 1, 40/scale, 0], [real(scale), 0, 0, 1, real(160*scale), 0]],
                   'bounds': [[-small, -500, small, 500], [-large, -500, large, 500]]})
    return result


def inspect(engine, path):
    image, characters = engine.render(path)
    dll = engine.dll
    generated = dll.FPDFText_IsGenerated
    generated.restype = c.c_int
    generated.argtypes = [c.c_void_p, c.c_int]
    document = dll.FPDF_LoadDocument(str(path.resolve()).encode('utf-8'), None)
    assert document
    page = text = None
    try:
        page = dll.FPDF_LoadPage(document, 0)
        assert page
        text = dll.FPDFText_LoadPage(page)
        assert text and dll.FPDFText_CountChars(text) == len(characters)
        physical = []
        for index, char in enumerate(characters):
            flag = generated(text, index)
            assert flag in [0, 1]
            if not flag:
                physical.append(char)
    finally:
        if text: dll.FPDFText_ClosePage(text)
        if page: dll.FPDF_ClosePage(page)
        dll.FPDF_CloseDocument(document)
    return {'text': ''.join(chr(char['unicode']) for char in characters), 'characters': characters,
            'physical': physical, 'pixelsSha256': hashlib.sha256(image.tobytes()).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dll', type=Path, required=True)
    parser.add_argument('--baseline-dll', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    inputs = cases()
    for case in inputs:
        for flat in [False, True]:
            fixture(args.output / f'{case["name"]}-{"flat" if flat else "nested"}.pdf', case, flat)
    report = {'scriptSha256': HELPERS['digest'](Path(__file__)), 'cases': inputs, 'renderers': {}, 'failures': []}
    for label, path in [('candidate', args.dll), ('baseline', args.baseline_dll)]:
        if path is None: continue
        engine = HELPERS['Pdfium'](path)
        try:
            records = {}
            for case in inputs:
                records[case['name']] = [inspect(engine, args.output / f'{case["name"]}-{mode}.pdf') for mode in ['nested', 'flat']]
        finally:
            engine.close()
        report['renderers'][label] = {'dllSha256': HELPERS['digest'](path), 'results': records}
    for case in inputs:
        name = case['name']; values = report['renderers']['candidate']['results'][name]
        def require(condition, reason):
            if not condition: report['failures'].append({'case': name, 'reason': reason})
        if 'expected' in case:
            require(all(value['text'] == case['expected'] for value in values), 'literal expected separator')
            require(values[0]['characters'] == values[1]['characters'], 'equivalent complete glyphs')
            require(values[0]['pixelsSha256'] == values[1]['pixelsSha256'], 'equivalent rendering')
        if args.baseline_dll:
            baseline = report['renderers']['baseline']['results'][name]
            for mode, before, after in zip(['nested', 'flat'], baseline, values):
                require(before['pixelsSha256'] == after['pixelsSha256'], f'{mode} baseline pixels')
                require(before['physical'] == after['physical'], f'{mode} baseline physical glyphs')
                if case.get('unchanged'):
                    require(before == after, f'{mode} excluded behavior')
    report['sourceHashes'] = {path.name: HELPERS['digest'](path) for path in sorted(args.output.glob('*.pdf'))}
    report['ok'] = not report['failures']
    (args.output / 'results.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps({'ok': report['ok'], 'cases': len(inputs), 'failures': report['failures']}))
    raise SystemExit(0 if report['ok'] else 1)


if __name__ == '__main__':
    main()
