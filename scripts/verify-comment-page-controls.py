"""Independently verify the native page-control desktop output."""
from pathlib import Path
import os, json, hashlib, sys
import fitz
from pypdf import PdfReader
root=Path(os.environ.get('FOLIO_COMMENT_PAGE_ROOT','artifacts/comment-page-desktop'))
if '--generate' in sys.argv:
 from pypdf import PdfWriter
 from pypdf.generic import DictionaryObject,NameObject,NumberObject,TextStringObject,ArrayObject
 root.mkdir(parents=True,exist_ok=True)
 source=root/'Page controls.pdf';assert not source.exists(), 'Preserve existing fixture'
 writer=PdfWriter(clone_from='artifacts/comment-fidelity-desktop/Comment fidelity.pdf')
 page=writer.pages[1]
 page[NameObject('/CropBox')]=ArrayObject([NumberObject(n) for n in [20,30,580,770]])
 page[NameObject('/Rotate')]=NumberObject(90)
 target=next(ref.get_object() for ref in page['/Annots'] if ref.get_object().get('/Contents')=='Review this paragraph')
 target[NameObject('/F')]=NumberObject(20)
 for flags,label in [(2,'Hidden note'),(32,'No-view note')]:
  note=DictionaryObject({NameObject('/Type'):NameObject('/Annot'),NameObject('/Subtype'):NameObject('/Text'),NameObject('/Rect'):ArrayObject([NumberObject(v) for v in [120,180,138,198]]),NameObject('/F'):NumberObject(flags),NameObject('/Contents'):TextStringObject(label)})
  page['/Annots'].append(writer._add_object(note))
 writer.write(source);print('Generated',source);sys.exit(0)
result=json.loads((root/'results.json').read_text(encoding='utf-8'))
assert result['passed']
source=root/'Page controls.pdf';output=root/'Folio edited.pdf'
assert hashlib.sha256(source.read_bytes()).hexdigest()==result['sourceHash']
a,b=PdfReader(source),PdfReader(output)
def notes(page):
 return {str(n.get('/Contents','')):n for ref in page.get('/Annots',[]) if (n:=ref.get_object()).get('/Subtype')=='/Text'}
def appearance(note):
 ap=note.get('/AP')
 return ap.get_object()['/N'].get_object().get_data() if ap else None
def popup(note):
 p=note.get('/Popup')
 return p.get_object() if p else None
assert len(a.pages)==len(b.pages)==2
for before_page,after_page in zip(a.pages,b.pages):
 before,after=notes(before_page),notes(after_page);assert before.keys()==after.keys()
 for label,old in before.items():
  new=after[label]
  for key in ['/Contents','/T','/Subj','/Name','/NM','/F','/C','/M','/CreationDate']:
   assert old.get(key)==new.get(key),(label,key)
  assert old['/Contents'].original_bytes==new['/Contents'].original_bytes
  assert appearance(old)==appearance(new),(label,'appearance ownership/content')
  if label!='Review this paragraph':assert old['/Rect']==new['/Rect'],label
  else:assert old['/Rect']!=new['/Rect']
  p,q=popup(old),popup(new);assert (p is None)==(q is None)
  if p is not None:
   assert p['/Rect']==q['/Rect']
   assert q['/Parent'].get_object()==new
with fitz.open(source) as original,fitz.open(output) as saved:
 assert original[1].rotation==90 and saved[1].rotation==180
 target=notes(b.pages[1])['Review this paragraph']
 # Numeric editing addresses PDF Rect. MuPDF's annotation.rect instead reports
 # the upright icon footprint for NoRotate, which is intentionally a separate box.
 crop=[float(n) for n in b.pages[1].cropbox];left,bottom,right,top=crop
 rect=[float(n) for n in target['/Rect']]
 assert saved[1].rotation==180
 actual=(right-max(rect[0],rect[2]),min(rect[1],rect[3])-bottom)
 assert abs(actual[0]-result['finalDisplay']['x'])<.02 and abs(actual[1]-result['finalDisplay']['y'])<.02,(actual,result['finalDisplay'])
 for i in range(2):
  original[i].set_rotation(saved[i].rotation)
  assert original[i].get_text()==saved[i].get_text()
  for scale in [1,2]:
   x=original[i].get_pixmap(matrix=fitz.Matrix(scale,scale),annots=False);y=saved[i].get_pixmap(matrix=fitz.Matrix(scale,scale),annots=False)
   assert (x.width,x.height,x.samples)==(y.width,y.height,y.samples),(i,scale)
report={'passed':True,'checks':['original unchanged','native note count including hidden notes preserved','Contents bytes and metadata preserved','custom appearances and popup geometry/parents retained','only moved note Rect changed','independent rotated/cropped destination matches UI','underlying pixels/text unchanged at 72 and 144 dpi']}
(root/'independent-results.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report))
