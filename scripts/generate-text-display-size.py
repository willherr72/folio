"""Synthetic desktop acceptance inputs; never reads private PDF content."""
from pathlib import Path
import hashlib,json
from pypdf import PdfWriter
from pypdf.generic import DictionaryObject as D,NameObject as N,ArrayObject as A,FloatObject as F,DecodedStreamObject

root=Path('artifacts/text-display-size-20261025/desktop');root.mkdir(parents=True,exist_ok=True)
def array(values):return A([F(v) for v in values])
inputs=[]
for kind,unit in [('direct',1),('group',1.7),('form',1)]:
    writer=PdfWriter();page=writer.add_blank_page(width=800,height=800)
    font=writer._add_object(D({N('/Type'):N('/Font'),N('/Subtype'):N('/Type1'),N('/BaseFont'):N('/Helvetica')}))
    fonts=D({N('/Font'):D({N('/F1'):font})})
    def stream(data,attrs=None):
        obj=DecodedStreamObject();obj.set_data(data)
        if attrs:obj.update(attrs)
        return writer._add_object(obj)
    if kind=='form':
        leaf=stream(b'BT /F1 1 Tf 1 0 0 1 5 10 Tm (Old) Tj ET',D({N('/Type'):N('/XObject'),N('/Subtype'):N('/Form'),N('/BBox'):array([0,0,100,30]),N('/Matrix'):array([2,0,0,2,0,0]),N('/Resources'):fonts}))
        wrapper=stream(b'/Leaf Do',D({N('/Type'):N('/XObject'),N('/Subtype'):N('/Form'),N('/BBox'):array([0,0,200,60]),N('/Matrix'):array([3,0,0,3,0,0]),N('/Resources'):D({N('/XObject'):D({N('/Leaf'):leaf})})}))
        resources=D({N('/XObject'):D({N('/Wrapper'):wrapper})})
        content=b'q 2 0 0 2 100 200 cm /Wrapper Do Q'
    elif kind=='group':
        resources=fonts;content=b'q 12 0 0 12 200 300 cm BT /F1 1 Tf 1 0 0 1 0 0 Tm (Old) Tj ( words) Tj ET Q'
    else:
        resources=fonts;content=b'BT /F1 1 Tf 12 0 0 12 200 300 Tm (Old) Tj ET'
    page[N('/Resources')]=resources;page[N('/Contents')]=stream(content);page[N('/UserUnit')]=F(unit)
    path=root/f'{kind}.pdf'
    if path.exists():raise RuntimeError(f'Preserving {path}')
    writer.write(path);inputs.append(dict(name=path.name,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),size=12*unit))
(root/'inputs.json').write_text(json.dumps(inputs,indent=2))
print('Generated three synthetic inputs')
