"""Independent text/style and pixel checks for the exported producer corpus."""
from pathlib import Path
import hashlib,json,re,sys
import fitz,numpy as np
from pypdf import PdfReader

root=Path(sys.argv[1] if len(sys.argv)>1 else 'artifacts/form-paragraph-corpus-20261025')
def pixels(page):
 p=page.get_pixmap(matrix=fitz.Matrix(2,2),alpha=False);return np.frombuffer(p.samples,dtype=np.uint8).reshape(p.height,p.width,3)
def glyphs(page,boundary):
 out=[]
 for block in page.get_text('rawdict')['blocks']:
  for line in block.get('lines',[]):
   for span in line['spans']:
    font=re.sub(r'^[A-Z]{6}\+','',span['font'])
    for char in span['chars']:
     if char['bbox'][0]<boundary and not char['c'].isspace():out.append((char['c'],font,round(span['size'],3),span['color']))
 return out
checks=[]
for group,modes in [('reportlab',['draw-string','text-lines','explicit-lines']),('platypus',['standard','embedded','mixed','imported'])]:
 for mode in modes:
  source=root/group/f'{mode}-original.pdf';saved=root/group/f'{mode}-edited.pdf'
  a=fitz.open(source);b=fitz.open(saved);assert len(a)==len(b)==2
  for reader in ['mupdf','pypdf']:
   text=b[0].get_text() if reader=='mupdf' else PdfReader(saved).pages[0].extract_text()
   text=' '.join(text.split());assert 'added paragraph text for reflow' in text,(mode,reader,text)
   assert text.count('First')==2 and text.count('Second')==2,(mode,reader)
   if mode in ['mixed','imported']:assert text.count('Caf'+chr(0xe9))==2 and chr(0xfffd) not in text,(mode,reader,'accent')
  boundary=300 if group=='reportlab' else 350
  before=glyphs(a[0],boundary);after=glyphs(b[0],boundary);assert after[:len(before)]==before,(mode,'retained glyph font/size/color changed')
  boundary=300 if group=='reportlab' else 350
  assert np.array_equal(pixels(a[0])[:,boundary*2:,:],pixels(b[0])[:,boundary*2:,:]),(mode,'sibling pixels changed')
  assert np.array_equal(pixels(a[1]),pixels(b[1])),(mode,'other page pixels changed')
  noop=root/group/f'{mode}-noop.pdf'
  if noop.exists():
   c=fitz.open(noop)
   for i in range(2):assert np.array_equal(pixels(b[i]),pixels(c[i])),(mode,'no-op pixels changed')
   c.close()
  checks.append({'case':f'{group}/{mode}','retainedNonspaceGlyphs':len(before),'textBothReaders':True,'sourceStylesRetained':True,'siblingAndOtherPagePixels':True,'unchangedReplayPixels':True if noop.exists() else None,'savedSha256':hashlib.sha256(saved.read_bytes()).hexdigest()})
  a.close();b.close()
for folder in ['form-text-flow','form-paragraph']:
 base=Path('tests/fixtures')/folder;manifest=json.loads((base/'manifest.json').read_text())
 for name,expected in manifest['files'].items():assert hashlib.sha256((base/name).read_bytes()).hexdigest()==expected,(folder,name)
output={'passed':True,'dpi':144,'checks':checks,'inputManifestsVerified':True,'limits':['Generated original fixtures, not an arbitrary real-world corpus. No printing/performance/accessibility claims.']}
with (root/'independent.json').open('x') as f:json.dump(output,f,indent=2)
print(f'PASS {len(checks)} producer cases: independent text/style and sibling/page pixels')
