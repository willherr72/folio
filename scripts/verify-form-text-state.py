"""Independent readers for generated nested-form text-state edit fixtures."""
import hashlib
import json
import math
import os
from pathlib import Path

import fitz
from PIL import Image, ImageChops, ImageDraw
from pypdf import PdfReader
from pypdf.generic import ContentStream

root = Path(os.environ.get('FOLIO_FORM_STATE_ARTIFACTS', 'artifacts/form-text-state-20261024/native'))
identity = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]

def matrix(values):
    a,b,c,d,e,f = map(float, values)
    return [[a,c,e], [b,d,f], [0.,0.,1.]]

def multiply(a,b):
    return [[sum(a[i][k]*b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]

def records(pdf):
    out = []
    def walk(stream, resources, ctm, state, path):
        state = state.copy()
        stack = []
        index = 0
        for args, op in ContentStream(stream, pdf).operations:
            if op == b'q': stack.append((ctm, state.copy()))
            elif op == b'Q': ctm, state = stack.pop()
            elif op == b'cm': ctm = multiply(ctm, matrix(args))
            elif op in (b'Tc', b'Tw', b'Tz', b'Ts', b'TL', b'Tr'): state[op.decode()] = float(args[0])
            elif op == b'BT': tm = identity
            elif op == b'Tm': tm = matrix(args)
            elif op == b'Tf': font = resources['/Font'][args[0]].get_object(); size = float(args[1])
            elif op == b'Tj':
                out.append(dict(path=path+[index], matrix=multiply(ctm,tm), state=state.copy(), font=font, size=size))
                index += 1
            elif op == b'Do':
                form = resources['/XObject'][args[0]].get_object()
                walk(form, form.get('/Resources', resources), multiply(ctm,matrix(form.get('/Matrix',[1,0,0,1,0,0]))),state,path+[index])
                index += 1
    p = pdf.pages[0]
    walk(p['/Contents'], p['/Resources'], identity, dict(Tc=0., Tw=0., Tz=100., Ts=0., TL=0., Tr=0.), [])
    return out

def raster(page):
    pix = page.get_pixmap(matrix=fitz.Matrix(2,2), colorspace=fitz.csRGB, alpha=False)
    return Image.frombytes('RGB', (pix.width,pix.height), pix.samples)

def equal_page(a,b):
    assert a.rect == b.rect and a.rotation == b.rotation and a.cropbox == b.cropbox
    assert raster(a).tobytes() == raster(b).tobytes()
    assert a.get_text('rawdict') == b.get_text('rawdict')

def spans(page):
    return [(line,span) for block in page.get_text('rawdict')['blocks'] if 'lines' in block for line in block['lines'] for span in line['spans']]

checks = []
outputs = sorted(root.glob('*-exported.pdf'))
assert len(outputs) == 15, len(outputs)
for output in outputs:
    prefix = output.name.removesuffix('-exported.pdf')
    reference = root / f'{prefix}-reference.pdf'
    original = root / f'{prefix}-original.pdf'
    with fitz.open(reference) as a, fitz.open(output) as b, fitz.open(original) as source:
        assert len(a) == len(b) == len(source) == 2
        for x,y in zip(a,b): equal_page(x,y)
        equal_page(source[1],b[1])
    before, after = records(PdfReader(original)), records(PdfReader(output))
    assert len(before) == len(after) == 3
    for a,b in zip(before,after):
        for key in ['path','matrix','state','size']: assert a[key] == b[key], (prefix,key,a[key],b[key])
    assert [p.extract_text() for p in PdfReader(reference).pages] == [p.extract_text() for p in PdfReader(output).pages]
    checks.append(prefix)

full_font = Path('tests/fixtures/corpus/fonts/DejaVuSerif.ttf').read_bytes()
for kind in ['standard','simple','cid']:
    original, output = root/f'{kind}-substitute-original.pdf', root/f'{kind}-substitute-edited.pdf'
    before, after = records(PdfReader(original)), records(PdfReader(output))
    assert len(before) == len(after) == 3
    for a,b in zip(before,after):
        for key in ['path','matrix','state','size']: assert a[key] == b[key], (kind,key)
    record = after[1]
    font = record['font']['/DescendantFonts'][0].get_object()
    assert font['/FontDescriptor']['/FontFile2'].get_data() == full_font
    widths = font['/W']
    widths = {int(widths[i]):float(widths[i+1][0]) for i in range(0,len(widths),2)}
    with fitz.open(original) as a, fitz.open(output) as b:
        old, new = spans(a[0]), spans(b[0])
        assert len(old) == len(new) == 3
        assert old[0] == new[0] and old[2] == new[2]
        line, span = new[1]
        assert ''.join(c['c'] for c in span['chars']) == 'Cafè edit'
        assert span['origin'] == old[1][1]['origin']
        assert span['color'] == old[1][1]['color'] and line['dir'] == old[1][0]['dir']
        x,y = span['origin']
        for glyph in span['chars']:
            assert abs(glyph['origin'][0]-x) < .002 and abs(glyph['origin'][1]-y) < .002, (kind,glyph,(x,y))
            advance = (math.floor(widths[ord(glyph['c'])]+.5)/1000*record['size']+record['state']['Tc'])*record['state']['Tz']/100
            x += record['matrix'][0][0]*advance
            y -= record['matrix'][1][0]*advance
        diff = ImageChops.difference(raster(a[0]),raster(b[0]))
        area = (fitz.Rect(old[1][1]['bbox']) | fitz.Rect(span['bbox'])) * a[0].rotation_matrix
        ImageDraw.Draw(diff).rectangle((area.x0*2-3,area.y0*2-3,area.x1*2+3,area.y1*2+3),fill=(0,0,0))
        assert diff.getbbox() is None
    checks.append(kind+' substitute state, glyph spacing and outside pixels')

report = dict(passed=True, readers=['MuPDF','pypdf'], referenceCases=len(outputs), substitutionCases=3,
              fontSha256=hashlib.sha256(full_font).hexdigest(), checks=checks)
(root/'independent-results.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report))
