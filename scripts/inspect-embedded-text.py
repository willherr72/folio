"""Inspect packaged embedded-font edit exports using two independent readers."""
from pathlib import Path
import hashlib,json
import fitz
from pypdf import PdfReader
root=Path('artifacts/embedded-text-desktop')
run=json.loads((root/'results.json').read_text())
original=fitz.open(root/'Moved original.pdf')
expected='Original sentence crème'
results=[]
for name in ['Editable.pdf','Flattened.pdf']:
 path=root/name
 pdf=fitz.open(path);page=pdf[0];reader=PdfReader(path)
 mupdf=page.get_text();pypdf=reader.pages[0].extract_text()
 for text in [expected,'Résumé café','Keep this neighbor']:
  assert text in mupdf and text in pypdf,(name,text,mupdf,pypdf)
 assert 'Original sentence\n' not in mupdf
 programs=[]
 for ref in reader.pages[0]['/Resources']['/Font'].values():
  font=ref.get_object()
  descendant=font['/DescendantFonts'][0].get_object() if '/DescendantFonts' in font else font
  if '/FontDescriptor' not in descendant:continue
  descriptor=descendant['/FontDescriptor'].get_object()
  if '/FontFile2' in descriptor:programs.append(hashlib.sha256(descriptor['/FontFile2'].get_object().get_data()).hexdigest())
 assert run['substituteFontId'] in programs,(name,programs)
 for text in ['Résumé café','Keep this neighbor']:
  before=original[0].search_for(text);after=page.search_for(text)
  assert len(before)==len(after)==1
  assert max(abs(a-b) for a,b in zip(before[0],after[0]))<.001,(name,text,before,after)
 # Changes are permitted only around the replaced run and the explicit added note.
 masks=original[0].search_for('Original sentence')+page.search_for(expected)
 if name=='Editable.pdf':
  annotations=list(page.annots() or [])
  assert len(annotations)==1
  masks.extend(a.rect for a in annotations)
 else:
  assert not list(page.annots() or [])
  masks.extend(page.search_for('Kept addition'))
 assert len(masks)>=3
 scale=2
 before=original[0].get_pixmap(matrix=fitz.Matrix(scale,scale),alpha=False)
 after=page.get_pixmap(matrix=fitz.Matrix(scale,scale),alpha=False)
 assert (before.width,before.height,before.n)==(after.width,after.height,after.n)
 a,b=before.samples,after.samples
 boxes=[(int(r.x0*scale)-6,int(r.y0*scale)-6,int(r.x1*scale)+7,int(r.y1*scale)+7) for r in masks]
 changed_outside=0
 for y in range(before.height):
  for x in range(before.width):
   if any(x0<=x<=x1 and y0<=y<=y1 for x0,y0,x1,y1 in boxes):continue
   offset=(y*before.width+x)*before.n
   if a[offset:offset+before.n]!=b[offset:offset+before.n]:changed_outside+=1
 assert changed_outside==0,(name,changed_outside)
 results.append({'file':name,'mupdfText':mupdf,'pypdfText':pypdf,'fontProgramSha256':programs,'outsideEditPixelsChanged':changed_outside})
 pdf.close()
report={'passed':True,'readers':{'PyMuPDF':fitz.VersionBind,'pypdf':__import__('pypdf').__version__},'files':results}
(root/'independent-readers.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False,indent=2))
