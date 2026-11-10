import hashlib,json,os
from pathlib import Path
import fitz
from PIL import Image,ImageChops,ImageDraw
from pypdf import PdfReader
root=Path(os.environ.get('FOLIO_PARAGRAPH_ALIGN_ROOT','artifacts/paragraph-align-acceptance'))
result=json.loads((root/'results.json').read_text());assert result['passed']
assert hashlib.sha256((root/'Original paragraph.pdf').read_bytes()).hexdigest()==result['sourceHash']
def spans(page):return [s for b in page.get_text('dict')['blocks'] if 'lines' in b for line in b['lines'] for s in line['spans']]
def span(page,text):return next(s for s in spans(page) if s['text']==text)
def image(page):
 p=page.get_pixmap(matrix=fitz.Matrix(2,2),colorspace=fitz.csRGB,alpha=False);return Image.frombytes('RGB',(p.width,p.height),p.samples)
def outside_equal(first,second,rects):
 a,b=image(first),image(second);assert a.size==b.size;diff=ImageChops.difference(a,b);assert diff.getbbox();draw=ImageDraw.Draw(diff)
 for r in rects:draw.rectangle((int(r.x0*2)-3,int(r.y0*2)-3,int(r.x1*2)+3,int(r.y1*2)+3),fill=(0,0,0))
 assert diff.getbbox() is None,diff.getbbox()
def rect(box):return fitz.Rect(box['x'],box['y'],box['x']+box['width'],box['y']+box['height'])
def notes(filename):return [(a.get_object().get('/Rect'),a.get_object().get('/F'),a.get_object().get('/Contents')) for a in PdfReader(root/filename).pages[0].get('/Annots',[]) if a.get_object().get('/Subtype')=='/Text']
before=fitz.open(root/'Before alignment.pdf');centered=fitz.open(root/'Centered paragraph.pdf');right=fitz.open(root/'Right paragraph.pdf');original=span(before[0],'Original words.');neighbor=span(before[0],'Neighbor stays.')
geometry=[]
for filename,doc,alignment in [('Centered paragraph.pdf',centered,'center'),('Right paragraph.pdf',right,'right')]:
 assert len(doc)==1 and doc[0].rotation==90
 text=PdfReader(root/filename).pages[0].extract_text();assert 'XXXXXXXXXX' in text and 'Neighbor stays.' in text and 'Original words.' not in text
 lines=[s for s in spans(doc[0]) if s['text'].strip() in ['XXXXXXXXXX','XX']];assert len(lines)==2
 # Measure X glyph ink, excluding trailing spaces that remain in the encoded text.
 boxes=[]
 for line in doc[0].get_text('rawdict')['blocks']:
  for row in line.get('lines',[]):
   chars=[c for s in row['spans'] for c in s['chars'] if c['c']=='X']
   if chars:boxes.append([min(c['bbox'][0] for c in chars),max(c['bbox'][2] for c in chars)])
 assert len(boxes)==2
 locations=[(a+b)/2 if alignment=='center' else b for a,b in boxes]
 expected=240 if alignment=='center' else 340
 assert max(locations)-min(locations)<0.1,(alignment,locations)
 assert all(abs(value-expected)<0.5 for value in locations),(alignment,locations)
 for line in lines:assert abs(line['size']-16)<0.01 and line['font']==original['font'] and line['color']==original['color']
 other=span(doc[0],'Neighbor stays.')
 for key in ['origin','bbox','size','font','color']:assert other[key]==neighbor[key]
 assert notes(filename)==notes('Before alignment.pdf');geometry.append({'alignment':alignment,'positions':locations,'expected':expected})
before[0].set_rotation(90)
outside_equal(before[0],centered[0],[rect(result[key])*before[0].rotation_matrix for key in ['old','new']])
outside_equal(centered[0],right[0],[rect(result['second'])])
report={'passed':True,'readers':['MuPDF','pypdf'],'geometry':geometry,'fontSizeColorPreserved':True,'neighborAndNoteUnchanged':True,'outsideEditedAreasPixelsUnchanged':True,'sourceUnchanged':True}
(root/'independent-results.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
