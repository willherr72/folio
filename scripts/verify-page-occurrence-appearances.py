"""Independent check of importer-colliding annotation appearance references."""
import argparse
import hashlib
import json
from pathlib import Path
import fitz
from pypdf import PdfReader

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--corpus',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
args.output.mkdir(parents=True,exist_ok=False)
digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
records=[]
for result in sorted(args.corpus.glob('ap-*-result.pdf')):
    label=result.name.removesuffix('-result.pdf')
    key=label.split('-')[1];selected=int(label.rsplit('-s',1)[1])
    source=args.corpus/f'{label}-source.pdf'
    paths=[source,result];hashes={p.name:digest(p) for p in paths}
    before,after=PdfReader(source),PdfReader(result)
    for index in [0,2]:
        note=after.pages[index]['/Annots'][1].get_object()
        form=note['/AP']['/N']['/Resources']['/XObject']['/'+key]
        assert form['/Type']=='/XObject' and form['/Subtype']=='/Form'
        assert form.get_data()==b'0.8 0.1 0.3 rg 0 0 20 20 re f'
    unchanged=[(1,1),(2 if selected==0 else 0,0)]
    with fitz.open(source) as old,fitz.open(result) as actual:
        for index,original in unchanged:
            assert actual[index].get_text()==old[original].get_text()
            for scale in [1,2]:
                a=actual[index].get_pixmap(matrix=fitz.Matrix(scale,scale),annots=True)
                b=old[original].get_pixmap(matrix=fitz.Matrix(scale,scale),annots=True)
                assert (a.width,a.height,a.samples)==(b.width,b.height,b.samples),(label,index,scale)
    assert hashes=={p.name:digest(p) for p in paths}
    records.append(dict(label=label,passed=True,sha256=hashes))
assert len(records)==6,len(records)
(args.output/'result.json').write_text(json.dumps(dict(passed=True,cases=records,mupdf=fitz.VersionBind),indent=2)+'\n',encoding='utf-8')
print(json.dumps(dict(passed=True,cases=len(records),unchangedPages=len(records)*2)))
