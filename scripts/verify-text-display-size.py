"""Independently read desktop outputs with MuPDF and pypdf."""
from pathlib import Path
import hashlib,json,math
import fitz
from pypdf import PdfReader
root=Path('artifacts/text-display-size-20261025/desktop')
run=json.loads((root/'results.json').read_text());assert run['passed']
results=[]
for check in run['checks']:
    kind=check['kind'];original=root/f'{kind}.pdf';edited=root/check['output']
    assert hashlib.sha256(original.read_bytes()).hexdigest()==check['originalSha256']
    source=fitz.open(original);output=fitz.open(edited)
    def spans(doc):return [s for b in doc[0].get_text('dict')['blocks'] if 'lines' in b for line in b['lines'] for s in line['spans']]
    before,after=spans(source),spans(output)
    assert before and after
    assert all(math.isclose(s['size'],check['size'],abs_tol=0.0001) for s in before+after),(kind,before,after)
    assert all(math.isclose(a,b,abs_tol=0.0001) for a,b in zip(before[0]['origin'],after[0]['origin'])),kind
    text='New words' if kind=='group' else 'New'
    assert text in output[0].get_text()
    assert text in PdfReader(edited).pages[0].extract_text()
    area=fitz.Rect(before[0]['bbox'])
    for span in before+after:area|=fitz.Rect(span['bbox'])
    area+=(-2,-2,2,2)
    a=source[0].get_pixmap();b=output[0].get_pixmap();assert (a.width,a.height,a.n)==(b.width,b.height,b.n)
    x0=max(0,int(area.x0));x1=min(a.width,math.ceil(area.x1));y0=max(0,int(area.y0));y1=min(a.height,math.ceil(area.y1))
    sa,sb=a.samples,b.samples;stride=a.stride
    for y in range(a.height):
        start=y*stride
        if y<y0 or y>=y1:assert sa[start:start+stride]==sb[start:start+stride],(kind,y)
        else:
            assert sa[start:start+x0*a.n]==sb[start:start+x0*a.n],(kind,y,'left')
            assert sa[start+x1*a.n:start+stride]==sb[start+x1*a.n:start+stride],(kind,y,'right')
    assert sa!=sb
    results.append(dict(kind=kind,physicalSize=after[0]['size'],origin=after[0]['origin'],text=text,outsideTextPixelsUnchanged=True))
(root/'independent-results.json').write_text(json.dumps(dict(passed=True,results=results),indent=2))
print('PASS independent physical sizes, positions, text, source hashes and surrounding pixels')
