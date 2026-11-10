"""Independent dense-page output references, built by changing one source show."""
import argparse,copy,json
from pathlib import Path
import fitz
from pypdf import PdfReader,PdfWriter
from pypdf.generic import NameObject,DecodedStreamObject,DictionaryObject

p=argparse.ArgumentParser(description=__doc__);p.add_argument('--run',type=Path,required=True);args=p.parse_args()
out=args.run/'independent';out.mkdir(exist_ok=False)
for kind in ['top','forms']:
    source=args.run/'workflow'/f'dense-100-{kind}.pdf';actual=args.run/'workflow'/f'{kind}-edited.pdf'
    writer=PdfWriter(clone_from=source);page=writer.pages[0];content=page.get_contents().get_data()
    if kind=='top':
        assert content.count(b'(Old)')==100;content=content.replace(b'(Old)',b'(New)',1)
    else:
        objects=page['/Resources']['/XObject'];leaf=objects['/Shared'];assert leaf.get_data().count(b'(Old)')==1
        replacement=DecodedStreamObject();replacement.update({key:copy.deepcopy(value) for key,value in leaf.items() if key not in ['/Length','/Filter','/DecodeParms']})
        replacement.set_data(leaf.get_data().replace(b'(Old)',b'(New)',1));objects[NameObject('/Edited')]=writer._add_object(replacement)
        assert content.count(b'/Shared Do')==100;content=content.replace(b'/Shared Do',b'/Edited Do',1)
    stream=DecodedStreamObject();stream.set_data(content);page[NameObject('/Contents')]=writer._add_object(stream)
    reference=out/f'{kind}-reference.pdf';writer.write(reference)
    with fitz.open(actual) as a,fitz.open(reference) as b:
        assert len(a)==len(b)==1
        assert a[0].get_text()==b[0].get_text(),(kind,'text')
        assert a[0].get_text().split().count('New')==1 and a[0].get_text().split().count('Old')==99
        for scale in [1,2]:
            x=a[0].get_pixmap(matrix=fitz.Matrix(scale,scale));y=b[0].get_pixmap(matrix=fitz.Matrix(scale,scale))
            assert (x.width,x.height,x.samples)==(y.width,y.height,y.samples),(kind,scale,'pixels')
    assert PdfReader(actual).pages[0].mediabox==PdfReader(source).pages[0].mediabox
report=dict(passed=True,documents=2,sourcePieces=100,changedPieces=1,exactText=True,authoredReferencePixelsAtScales=[1,2],frames=True)
(out/'result.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8');print(json.dumps(report))
