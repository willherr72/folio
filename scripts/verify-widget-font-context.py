from pathlib import Path
import hashlib,json,os
import fitz
from pypdf import PdfReader

root=Path(os.environ.get('FOLIO_WIDGET_FONT_ROOT','artifacts/widget-font-context-20261025/desktop'))
read=lambda p:json.loads(p.read_text(encoding='utf-8-sig'))
run=read(root/'results.json');assert run['passed']
source=root/'Text sample.pdf';assert hashlib.sha256(source.read_bytes()).hexdigest()==run['sourceHash']==read(root/'input.json')['sha256']
original=PdfReader(source);assert '/DR' not in original.trailer['/Root']['/AcroForm']
sources={name:fitz.open(root/name) for name in ['Text sample.pdf','Edited.pdf','Filled.pdf','Reedited.pdf']}
def raster(doc,page,clip=None):
    return doc[page].get_pixmap(matrix=fitz.Matrix(2,2),clip=clip,annots=True).samples
fields=list(sources['Text sample.pdf'][0].widgets());customer=next(w for w in fields if w.field_name=='Customer')
for name,text,value in [('Edited.pdf','Edited page','Ada'),('Filled.pdf','Edited page','Grace'),('Reedited.pdf','Final page','Grace')]:
    pdf=PdfReader(root/name);assert len(pdf.pages)==2
    assert text in pdf.pages[0].extract_text();assert pdf.get_fields()['Customer']['/V']==value
    assert pdf.get_fields()['Password']['/V']=='password-value'
    assert raster(sources[name],1)==raster(sources['Text sample.pdf'],1)
for widget in fields:
    assert raster(sources['Text sample.pdf'],0,widget.rect)==raster(sources['Edited.pdf'],0,widget.rect),widget.field_name
    assert raster(sources['Filled.pdf'],0,widget.rect)==raster(sources['Reedited.pdf'],0,widget.rect),widget.field_name
before=sources['Edited.pdf'][0].get_pixmap(matrix=fitz.Matrix(2,2),annots=True)
after=sources['Filled.pdf'][0].get_pixmap(matrix=fitz.Matrix(2,2),annots=True)
assert (before.width,before.height,before.n)==(after.width,after.height,after.n)
box=fitz.Rect(customer.rect);box.x0-=1;box.y0-=1;box.x1+=1;box.y1+=1
changed=0
a,b=before.samples,after.samples
for y in range(before.height):
    for x in range(before.width):
        offset=(y*before.width+x)*before.n
        if a[offset:offset+before.n]!=b[offset:offset+before.n]:
            changed+=1;assert box.contains(fitz.Point(x/2,y/2)),(x,y)
assert changed>0
result={'passed':True,'originalHasNoDefaultResources':True,'sourceUnchanged':True,'readers':['MuPDF','pypdf'],'fieldPixelsPreservedAcrossTextEdits':True,'filledValueSavedAndReedited':True,'unrelatedPagePixelsUnchanged':True,'fillPixelsChangedOnlyInsideCustomer':True,'changedFillPixels':changed,'limits':'Controlled two-page producer fixture; not general form or font parity.'}
(root/'independent-results.json').write_text(json.dumps(result,indent=2),encoding='utf-8');print(json.dumps(result,indent=2))
