"""Independent saved-text typography and outside-area pixel checks."""
import hashlib,json,os
from pathlib import Path
import fitz
from PIL import Image,ImageChops,ImageDraw
from pypdf import PdfReader
root=Path(os.environ.get('FOLIO_PARAGRAPH_SIZE_ROOT','artifacts/paragraph-size-acceptance'))
result=json.loads((root/'results.json').read_text());assert result['passed']
assert hashlib.sha256((root/'Original paragraph.pdf').read_bytes()).hexdigest()==result['sourceHash']
def span(page,text):return next(s for b in page.get_text('dict')['blocks'] if 'lines' in b for line in b['lines'] for s in line['spans'] if s['text']==text)
def image(page):
    p=page.get_pixmap(matrix=fitz.Matrix(2,2),colorspace=fitz.csRGB,alpha=False)
    return Image.frombytes('RGB',(p.width,p.height),p.samples)
def outside_equal(first,second,rects):
    a,b=image(first),image(second);assert a.size==b.size
    difference=ImageChops.difference(a,b);assert difference.getbbox()
    draw=ImageDraw.Draw(difference)
    for r in rects:draw.rectangle((int(r.x0*2)-3,int(r.y0*2)-3,int(r.x1*2)+3,int(r.y1*2)+3),fill=(0,0,0))
    assert difference.getbbox() is None,difference.getbbox()
def rect(box):return fitz.Rect(box['x'],box['y'],box['x']+box['width'],box['y']+box['height'])
before=fitz.open(root/'Before resize.pdf');larger=fitz.open(root/'Larger paragraph.pdf');smaller=fitz.open(root/'Smaller paragraph.pdf')
assert len(before)==len(larger)==len(smaller)==1
assert larger[0].rotation==smaller[0].rotation==90
original=span(before[0],'Original words.');neighbor=span(before[0],'Neighbor stays.')
def note_geometry(filename):
    # MuPDF synthesizes Text icon bounds according to page rotation. Compare
    # saved native annotation data here; render both pages at the same rotation below.
    return [(a.get_object().get('/Rect'),a.get_object().get('/F')) for a in PdfReader(root/filename).pages[0].get('/Annots',[]) if a.get_object().get('/Subtype')=='/Text']
for filename,document,size in [('Larger paragraph.pdf',larger,18),('Smaller paragraph.pdf',smaller,10)]:
    for text in [document[0].get_text(),PdfReader(root/filename).pages[0].extract_text()]:
        assert text.count('Sized paragraph.')==1 and text.count('Neighbor stays.')==1 and 'Original words.' not in text
    changed=span(document[0],'Sized paragraph.');assert abs(changed['size']-size)<0.01
    assert changed['font']==original['font'] and changed['color']==original['color']
    other=span(document[0],'Neighbor stays.')
    for key in ['origin','bbox','size','font','color']:assert other[key]==neighbor[key],(filename,key)
    assert [a.info['content'] for a in document[0].annots()]==['Keep size note']
    assert note_geometry(filename)==note_geometry('Before resize.pdf')
before[0].set_rotation(90)
outside_equal(before[0],larger[0],[rect(result[key])*before[0].rotation_matrix for key in ['old','new']])
outside_equal(larger[0],smaller[0],[rect(result['second'])])
report={'passed':True,'readers':['MuPDF','pypdf'],'savedSizes':[18,10],'fontAndColorPreserved':True,'neighborGeometryAndStyleUnchanged':True,'nativeNotePreserved':True,'outsideEditedAreasPixelsUnchanged':True,'sourceUnchanged':True,'rotation':90}
(root/'independent-results.json').write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
