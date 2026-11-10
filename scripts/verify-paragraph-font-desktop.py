import hashlib,json,os
from pathlib import Path
import fitz
from PIL import Image,ImageChops,ImageDraw
from pypdf import PdfReader
root=Path(os.environ.get('FOLIO_PARAGRAPH_FONT_ROOT','artifacts/paragraph-font-acceptance'))
result=json.loads((root/'results.json').read_text());assert result['passed']
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(root/'Annotated subset.pdf')==result['sourceHash']
assert sha(root/'Unavailable substitute.ttf')==result['fontHash']
def spans(page):return [s for b in page.get_text('dict')['blocks'] for line in b.get('lines',[]) for s in line['spans']]
def span(page,text):return next(s for s in spans(page) if s['text']==text)
def notes(filename):return [(a.get_object().get('/Rect'),a.get_object().get('/F'),a.get_object().get('/Contents')) for a in PdfReader(root/filename).pages[0].get('/Annots',[]) if a.get_object().get('/Subtype')=='/Text']
def rect(box):return fitz.Rect(box['x'],box['y'],box['x']+box['width'],box['y']+box['height'])
def image(page):
 p=page.get_pixmap(matrix=fitz.Matrix(2,2),colorspace=fitz.csRGB,alpha=False);return Image.frombytes('RGB',(p.width,p.height),p.samples)
def outside_equal(first,second,rects):
 a,b=image(first),image(second);assert a.size==b.size;diff=ImageChops.difference(a,b);assert diff.getbbox();draw=ImageDraw.Draw(diff)
 for r in rects:draw.rectangle((int(r.x0*2)-3,int(r.y0*2)-3,int(r.x1*2)+3,int(r.y1*2)+3),fill=(0,0,0))
 assert diff.getbbox() is None,diff.getbbox()
before=fitz.open(root/'Before font.pdf');edited=fitz.open(root/'Font paragraph.pdf');again=fitz.open(root/'Reedited font paragraph.pdf')
original=span(before[0],'Café old');neighbor=span(before[0],'Neighbor');geometry=[]
for name,doc,replacement in [('Font paragraph.pdf',edited,'Cafè new'),('Reedited font paragraph.pdf',again,'Cafè again')]:
 assert len(doc)==1 and doc[0].rotation==90
 reader=PdfReader(root/name);text=reader.pages[0].extract_text();assert replacement in text and 'Café old' not in text and 'Neighbor' in text
 changed=[span(doc[0],value) for value in [replacement,'jj']]
 # The shared embedder deliberately names resources Folio + program SHA256.
 # MuPDF truncates the reported name; verify that prefix and the exact embedded
 # program below rather than expecting the font's human-readable family name.
 assert all(abs(s['size']-16)<0.01 and s['color']==original['color'] and len(s['font'])>=20 and ('Folio'+result['fontHash']).startswith(s['font']) for s in changed),changed
 ys=sorted(s['origin'][1] for s in changed);assert abs(ys[1]-ys[0]-24)<0.02
 # MuPDF span boxes include advances (and bearings); compare right edges to
 # the requested raw PDF area, with tolerance for reader metric rounding.
 rights=[s['bbox'][2] for s in changed];assert max(rights)-min(rights)<0.2,rights;assert all(abs(x-160)<0.2 for x in rights),rights
 other=span(doc[0],'Neighbor')
 for key in ['origin','bbox','size','font','color']:assert other[key]==neighbor[key]
 assert notes(name)==notes('Before font.pdf') and len(notes(name))==1
 fonts=reader.pages[0]['/Resources']['/Font'];programs=[]
 for f in fonts.values():
  f=f.get_object()
  for desc in f.get('/DescendantFonts',[]):
   descriptor=desc.get_object().get('/FontDescriptor')
   if descriptor and '/FontFile2' in descriptor.get_object():programs.append(hashlib.sha256(descriptor.get_object()['/FontFile2'].get_data()).hexdigest())
 assert result['fontHash'] in programs
 geometry.append({'file':name,'baselineDistance':ys[1]-ys[0],'rightEdges':rights,'reportedFont':changed[0]['font'],'size':16,'fontProgramSha256':result['fontHash']})
before[0].set_rotation(90)
outside_equal(before[0],edited[0],[rect(result[key])*before[0].rotation_matrix for key in ['old','new']])
outside_equal(edited[0],again[0],[rect(result['second'])])
record={'passed':True,'readers':['MuPDF','pypdf'],'geometry':geometry,'neighborAndNativeNotePreserved':True,'outsideEditedAreasPixelsUnchanged':True,'sourceAndFontUnchanged':True}
(root/'independent-results.json').write_text(json.dumps(record,indent=2));print(json.dumps(record,indent=2))
