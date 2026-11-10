"""Check the supplied quiz word insertion with independent PDF readers and pixels."""
import os,json,hashlib,math
from pathlib import Path
import fitz
from pypdf import PdfReader
from PIL import Image,ImageChops,ImageDraw
root=Path(os.environ['FOLIO_QUIZ_ACCEPTANCE_ROOT']);result=json.loads((root/'results.json').read_text())
assert result['passed'];assert hashlib.sha256((root/'Quiz.pdf').read_bytes()).hexdigest()==result['sourceHash']
a=fitz.open(root/'Quiz.pdf');b=fitz.open(root/'Folio edited.pdf');assert len(a)==len(b)
normalized=lambda value:' '.join(value.split())
for text in [b[0].get_text(),PdfReader(root/'Folio edited.pdf').pages[0].extract_text()]:
 assert normalized(result['replacement']) in normalized(text)
 assert normalized(result['original']) not in normalized(text)
area=result['area'];changed=0
for index in range(len(a)):
 before=a[index].get_pixmap(matrix=fitz.Matrix(2,2),alpha=False);after=b[index].get_pixmap(matrix=fitz.Matrix(2,2),alpha=False)
 assert (before.width,before.height)==(after.width,after.height)
 difference=ImageChops.difference(Image.frombytes('RGB',(before.width,before.height),before.samples),Image.frombytes('RGB',(after.width,after.height),after.samples))
 if index==0:
  assert difference.getbbox() is not None
  changed=sum(pixel!=(0,0,0) for pixel in difference.getdata())
  ImageDraw.Draw(difference).rectangle([math.floor(area['x']*2)-2,math.floor(area['y']*2)-2,math.ceil((area['x']+area['width'])*2)+2,math.ceil((area['y']+area['height'])*2)+2],fill=(0,0,0))
 assert difference.getbbox() is None,(index,'pixels outside paragraph changed')
spans=[s for block in b[0].get_text('dict')['blocks'] if 'lines' in block for line in block['lines'] for s in line['spans'] if s['text'].strip() and area['x']-1<=s['bbox'][0]<area['x']+area['width'] and area['y']-1<=s['bbox'][1]<area['y']+area['height']]
assert spans and all('Calibri' in s['font'] and abs(s['size']-12)<.01 and s['color']==0 for s in spans)
report={'passed':True,'independentReaders':['MuPDF','pypdf'],'font':'Calibri','size':12,'outsideChangedPixels':0,'changedPixels':changed,'originalUnchanged':True}
(root/'independent-results.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report))
