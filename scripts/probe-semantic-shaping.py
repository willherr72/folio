"""Development experiment: exact native outlines plus one Unicode semantic layer."""
import argparse, hashlib, io, json, pathlib, subprocess, sys
from pypdf import PdfReader, PdfWriter
from pypdf.generic import DictionaryObject, NameObject, NumberObject, FloatObject, ArrayObject, DecodedStreamObject
from fontTools.ttLib import TTFont
from fontTools.pens.basePen import BasePen
ROOT = pathlib.Path(__file__).resolve().parents[1]

def D(**kw):
    return DictionaryObject({NameObject('/' + k): v for k, v in kw.items()})

def N(s):
    return NameObject('/' + s)

def nums(values):
    return ArrayObject([FloatObject(v) for v in values])

def stream(w, b):
    s = DecodedStreamObject()
    s.set_data(b)
    return w._add_object(s)

class Pen(BasePen):

    def __init__(self, gs):
        super().__init__(gs)
        self.ops = []

    def _moveTo(self, p):
        self.ops.append(f'{p[0]} {p[1]} m')

    def _lineTo(self, p):
        self.ops.append(f'{p[0]} {p[1]} l')

    def _curveToOne(self, a, b, c):
        self.ops.append(' '.join(map(str, [*a, *b, *c])) + ' c')

    def _closePath(self):
        self.ops.append('h')

    def _endPath(self):
        pass
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--input', type=pathlib.Path, default=ROOT / 'artifacts/shaped-text/native')
parser.add_argument('--output', type=pathlib.Path, default=ROOT / 'artifacts/shaped-text/semantic')
parser.add_argument('--cluster-boxes', action='store_true')
parser.add_argument('--python-deps', type=pathlib.Path, default=ROOT / 'artifacts/complex-text/python')
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
w = PdfWriter()
records = []
for case in ['ligatures-on', 'decomposed', 'two-axis-marks', 'arabic', 'mixed-bidi', 'indic', 'supplementary']:
    layout = json.loads((args.input / f'{case}-0.layout.json').read_text())
    reader = PdfReader(args.input / f'{case}-0.pdf')
    original = reader.pages[0]
    font = next(iter(original['/Resources']['/Font'].values())).get_object()
    fb = font['/DescendantFonts'][0].get_object()['/FontDescriptor']['/FontFile2'].get_data()
    assert hashlib.sha256(fb).hexdigest() == layout['fontId'], 'Native font identity mismatch'
    tt = TTFont(io.BytesIO(fb))
    gs = tt.getGlyphSet()
    upem = tt['head'].unitsPerEm
    size = layout['fontSize']
    scale = size / upem
    bounds = layout['bounds']
    origin = (48 - min(0, bounds['xMin']), 48 - min(0, bounds['yMin']))
    visible = ['q 0.1 0.2 0.3 rg']
    clusters = {}
    for run in layout['runs']:
        for g in run['glyphs']:
            key = (g['cluster']['utf8Start'], g['cluster']['utf8End'])
            clusters.setdefault(key, {'glyphs': [], 'rtl': run['direction'] == 'rtl'})['glyphs'].append(g)
            pen = Pen(gs)
            gs[tt.getGlyphName(g['glyphId'])].draw(pen)
            visible.append(f"q {scale} 0 0 {scale} {origin[0] + g['x']} {origin[1] + g['y']} cm " + ' '.join(pen.ops) + ' f Q')
    visible.append('Q')
    cells = []
    for (start, end), cluster in sorted(clusters.items()):
        text = layout['text'].encode()[start:end].decode()
        glyphs = cluster['glyphs']
        left = min((g['x'] - g['xOffset'] for g in glyphs))
        advance = sum((g['xAdvance'] for g in glyphs))
        b = [g['bounds'] for g in glyphs if g['bounds']]
        ymin = min((g['yMin'] for g in b), default=-size * 0.2)
        ymax = max((g['yMax'] for g in b), default=size * 0.8)
        count = len(text)
        width = advance / count
        for i, char in enumerate(text):
            x = left + width * (count - i - 1 if cluster['rtl'] else i)
            cells.append(dict(char=char, x=x, width=width, ymin=ymin, ymax=ymax, bxmin=min([left] + [g['xMin'] for g in b]), bxmax=max([left + advance] + [g['xMax'] for g in b])))
    assert len(cells) <= 255, 'Prototype Type3 scalar limit exceeded'
    for rotation, order in [(r, o) for r in [0, 90, 180, 270] for o in ['logical', 'visual', 'logical-tj', 'visual-tj']]:
        page = w.add_blank_page(float(original.mediabox.width), float(original.mediabox.height))
        page[N('Rotate')] = NumberObject(rotation)
        charprocs = D()
        differences = ArrayObject([NumberObject(1)])
        widths = []
        cmap = []
        for i, cell in enumerate(cells, 1):
            widths.append(cell['width'] / size * 1000)
            differences.append(N(f'g{i}'))
            bxmin = (cell['bxmin'] - cell['x']) / size * 1000 if args.cluster_boxes else 0
            bxmax = (cell['bxmax'] - cell['x']) / size * 1000 if args.cluster_boxes else widths[-1]
            charprocs[N(f'g{i}')] = stream(w, f"{widths[-1]:.7f} 0 {bxmin:.7f} {cell['ymin'] / size * 1000:.7f} {bxmax:.7f} {cell['ymax'] / size * 1000:.7f} d1".encode())
            cmap.append(f"<{i:02X}> <{cell['char'].encode('utf-16-be').hex()}>")
        mapping = '/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /Semantic def /CMapType 2 def 1 begincodespacerange <00> <FF> endcodespacerange ' + str(len(cmap)) + ' beginbfchar ' + ' '.join(cmap) + ' endbfchar endcmap CMapName currentdict /CMap defineresource pop end end'
        semantic = w._add_object(D(Type=N('Font'), Subtype=N('Type3'), FontBBox=nums([0, -1000, 2000, 2000]), FontMatrix=nums([0.001, 0, 0, 0.001, 0, 0]), CharProcs=charprocs, Encoding=D(Type=N('Encoding'), Differences=differences), FirstChar=NumberObject(1), LastChar=NumberObject(len(cells)), Widths=nums(widths), Resources=D(), ToUnicode=stream(w, mapping.encode())))
        cmds = visible + ['BT /S ' + str(size) + ' Tf']
        ordered = list(enumerate(cells, 1))
        if order.startswith('visual'):
            ordered.sort(key=lambda c: c[1]['x'])
        if order.endswith('tj'):
            cmds.append(f"1 0 0 1 {origin[0] + ordered[0][1]['x']} {origin[1]} Tm")
            tj = []
            for j, (i, c) in enumerate(ordered):
                tj.append(f'<{i:02X}>')
                if j + 1 < len(ordered):
                    tj.append(format((c['x'] + c['width'] - ordered[j + 1][1]['x']) / size * 1000, '.7f'))
            cmds.append('[' + ' '.join(tj) + '] TJ')
        else:
            for i, c in ordered:
                cmds.append(f"1 0 0 1 {origin[0] + c['x']} {origin[1]} Tm <{i:02X}> Tj")
        cmds.append('ET')
        page[N('Resources')] = D(Font=D(S=semantic))
        page[N('Contents')] = stream(w, '\n'.join(cmds).encode())
        records.append(dict(case=case, rotation=rotation, order=order, text=layout['text'], cells=cells))
    tt.close()
w.write(args.output / 'semantic.pdf')
(args.output / 'inputs.json').write_text(json.dumps(records, ensure_ascii=True, indent=2))
subprocess.run([sys.executable, str(ROOT / 'scripts/probe-shaped-text-interop.py'), '--python-deps', str(args.python_deps), '--inspect', str(args.output / 'semantic.pdf'), '--output', str(args.output / 'inspection')], check=True)
