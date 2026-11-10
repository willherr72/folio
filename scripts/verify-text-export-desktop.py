from pathlib import Path
import hashlib,json,os
from pypdf import PdfReader
root=Path(os.environ.get('FOLIO_TEXT_EXPORT_ROOT','artifacts/text-export-20261024/desktop-final'))
read=lambda p:json.loads(p.read_text(encoding='utf-8-sig'))
run=read(root/'results.json');assert run['passed']
source=root/'Text sample.pdf';digest=hashlib.sha256(source.read_bytes()).hexdigest()
assert digest==run['sourceHash']==read(root/'input.json')['sha256']
pdf=PdfReader(source);assert len(pdf.pages)==2
assert pdf.get_fields()['Customer']['/V']=='Ada'
assert 'First page' in pdf.pages[0].extract_text()
assert 'Cropped away' in pdf.pages[0].extract_text()
expected={'all.txt':['Edited page Unsaved overlay [Form fields] Customer: Ada','Second page'], 'range.txt':['Second page']}
results=[]
for filename,pages in expected.items():
 data=(root/filename).read_bytes();assert data.startswith(b'\xef\xbb\xbf')
 text=data.decode('utf-8-sig');assert '\r' not in text
 actual=[' '.join(page.split()) for page in text.split('\f')]
 assert actual==pages,(filename,actual,pages)
 assert 'password-value' not in text and 'Cropped away' not in text
 results.append({'file':filename,'sha256':hashlib.sha256(data).hexdigest(),'pages':len(pages),'text':actual})
assert not (root/'cancelled.txt').exists()
assert not list(root.glob('.folio-text-*'))
result={'passed':True,'sourceUnchanged':True,'strictUtf8BomAndLf':True,'expectedEditedTextAndFormValues':True,'pageOrderAndRange':True,'passwordAndCroppedTextOmitted':True,'cancelProducedNoOutput':True,'outputs':results,'limits':'Controlled fixture; not a general reading-order or visual-redaction oracle.'}
(root/'independent-results.json').write_text(json.dumps(result,indent=2),encoding='utf-8');print(json.dumps(result,indent=2))
