from pathlib import Path
import hashlib,json,os
import fitz,numpy as np
from PIL import Image
from pypdf import PdfReader
root=Path(os.environ.get('FOLIO_CANVAS_FORM_ROOT','artifacts/canvas-form-filling-20261024/desktop'))
read=lambda path:json.loads(path.read_text(encoding='utf-8-sig'))
results=read(root/'results.json');assert results['passed']
assert hashlib.sha256((root/'Form.pdf').read_bytes()).hexdigest()==read(root/'input.json')['sha256']
source=PdfReader(root/'Form.pdf');saved=PdfReader(root/'Folio edited.pdf');fields=saved.get_fields()
expected={'full_name':'Canvas selected name','consent':'/Yes','country':'ca','contact':'postal','interests':['music','science'],'comments':'First line\nSecond line','delivery':'/Mail'}
for name,value in expected.items():
    actual=fields[name]['/V']
    if isinstance(actual,str):actual=actual.replace('\r\n','\n').replace('\r','\n')
    assert actual==value,(name,actual)
for name,field in source.get_fields().items():
    if name not in expected:assert fields[name].get('/V')==field.get('/V'),name
assert saved.pages[0].rotation==180 and len(saved.pages)==2
geometry=[]
for entry in results['geometry']:
    pixels=np.asarray(Image.open(root/(entry['name']+'-native.png')).convert('RGB'))
    ys,xs=np.where((pixels[:,:,0]<40)&(pixels[:,:,1]>180)&(pixels[:,:,2]>180))
    assert len(xs)>30,(entry['name'],'No cyan appearance')
    _,_,width,height=map(float,entry['viewBox'].split())
    actual={'x':float(xs.min())*width/pixels.shape[1],'y':float(ys.min())*height/pixels.shape[0],
            'width':float(xs.max()-xs.min()+1)*width/pixels.shape[1],'height':float(ys.max()-ys.min()+1)*height/pixels.shape[0]}
    for key in actual:assert abs(actual[key]-entry['bounds'][key])<2.5,(entry['name'],key,actual,entry['bounds'])
    geometry.append({'name':entry['name'],'nativeAppearance':actual,'target':entry['bounds']})
counts=[]
with fitz.open(root/'Form.pdf') as original,fitz.open(root/'Folio edited.pdf') as output:
    original[0].set_rotation(180)
    for index,page in enumerate(output):
        before=original[index];a=before.get_pixmap(alpha=False);b=page.get_pixmap(alpha=False)
        first=np.frombuffer(a.samples,np.uint8).reshape(a.height,a.width,3);second=np.frombuffer(b.samples,np.uint8).reshape(b.height,b.width,3)
        assert first.shape==second.shape
        allowed=np.zeros(first.shape[:2],bool)
        for widget in before.widgets():
            rect=widget.rect*before.rotation_matrix
            allowed[max(0,int(rect.y0)-3):min(a.height,int(rect.y1)+4),max(0,int(rect.x0)-3):min(a.width,int(rect.x1)+4)]=True
        for annotation in before.annots() or []:
            rect=annotation.rect*before.rotation_matrix
            allowed[max(0,int(rect.y0)-3):min(a.height,int(rect.y1)+4),max(0,int(rect.x0)-3):min(a.width,int(rect.x1)+4)]=True
        changed=np.any(first!=second,axis=2);assert not np.any(changed&~allowed),(index,int(np.count_nonzero(changed&~allowed)))
        assert any(widget.field_name=='full_name' and widget.field_value==expected['full_name'] for widget in page.widgets())
        assert len(list(page.annots() or []))==len(list(before.annots() or []))
        counts.append(int(np.count_nonzero(changed)))
record={'passed':True,'fields':expected,'readers':['MuPDF','pypdf'],'sourceUnchanged':True,'sharedNameUpdatedOnBothPages':True,'outsideAnnotationPixelsUnchanged':True,'changedPixels':counts,'noRotateNativeAppearanceMatchesTargets':geometry}
(root/'independent-results.json').write_text(json.dumps(record,indent=2));print(json.dumps(record,indent=2))
