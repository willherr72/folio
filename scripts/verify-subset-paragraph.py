"""Independent MuPDF/pypdf checks of saved subset paragraph edits and replay."""
import argparse, hashlib, json
from pathlib import Path
import fitz
from pypdf import PdfReader

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--corpus',type=Path,required=True)
parser.add_argument('--out',type=Path,required=True)
parser.add_argument('--desktop',action='store_true')
args=parser.parse_args();args.out.mkdir(exist_ok=False)
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def fonts(path):
    values=[]
    for font in PdfReader(path).pages[0]['/Resources']['/Font'].values():
        font=font.get_object()
        if '/DescendantFonts' in font: font=font['/DescendantFonts'][0].get_object()
        descriptor=font.get('/FontDescriptor')
        if descriptor and '/FontFile2' in descriptor.get_object():
            values.append(hashlib.sha256(descriptor.get_object()['/FontFile2'].get_data()).hexdigest())
    return sorted(set(values))
def spans(page): return [span for block in page.get_text('dict')['blocks'] if 'lines' in block for line in block['lines'] for span in line['spans']]
results=[]
for rotation in [0,90,180,270]:
    for mode in ([''] if args.desktop else ['-uniform','-styled']):
        source=args.corpus/f'source-{rotation}{mode}.pdf'
        saved=args.corpus/f'saved-{rotation}{mode}.pdf'
        repeated=args.corpus/f'repeated-{rotation}{mode}.pdf'
        assert fonts(source)==fonts(saved)==fonts(repeated),(rotation,mode,'embedded font programs')
        with fitz.open(source) as original,fitz.open(saved) as edited,fitz.open(repeated) as replay:
            assert len(original)==len(edited)==len(replay)==1
            assert original[0].rect==edited[0].rect==replay[0].rect
            assert original[0].rotation==edited[0].rotation==replay[0].rotation==rotation
            assert sorted(edited[0].get_text().split())==['Edit','Neighbor']
            assert edited[0].get_text()==replay[0].get_text()
            for scale in [1,2]:
                a=edited[0].get_pixmap(matrix=fitz.Matrix(scale,scale));b=replay[0].get_pixmap(matrix=fitz.Matrix(scale,scale))
                assert (a.width,a.height,a.samples)==(b.width,b.height,b.samples),(rotation,mode,'exact replay pixels',scale)
            # Compare the untouched half of the physical page in canonical units.
            original[0].set_rotation(0);edited[0].set_rotation(0)
            old=next(s for s in spans(original[0]) if s['text'].strip()=='Neighbor')
            new=next(s for s in spans(edited[0]) if s['text'].strip()=='Neighbor')
            for key in ['text','origin','bbox','font','size','color']: assert old[key]==new[key],(rotation,mode,'neighbor',key)
            replacement=next(s for s in spans(edited[0]) if s['text'].strip()=='Edit')
            selected=next(s for s in spans(original[0]) if s['text'].strip()=='Old')
            for key in ['font','size','color']: assert replacement[key]==selected[key],(rotation,mode,'selected style',key)
            assert 29.9<=replacement['bbox'][0]<replacement['bbox'][2]<140
            for scale in [1,2]:
                clip=fitz.Rect(140,0,300,400)
                a=original[0].get_pixmap(matrix=fitz.Matrix(scale,scale),clip=clip);b=edited[0].get_pixmap(matrix=fitz.Matrix(scale,scale),clip=clip)
                assert a.samples==b.samples,(rotation,mode,'untouched pixels',scale)
        results.append(dict(rotation=rotation,mode=mode or 'desktop',sourceSha256=sha(source),savedSha256=sha(saved),repeatedSha256=sha(repeated),fontPrograms=fonts(saved),exactReplayPixels=[1,2],exactNeighborPixels=[1,2],textAndStyle=True))
report=dict(passed=True,savedOutputs=2*len(results),results=results,limits=['No independently computed paragraph line-breaking oracle','Controlled CID subset with one selected text piece'])
(args.out/'result.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps(dict(passed=True,savedOutputs=report['savedOutputs'])))
