"""Independently verify saved shared field values after occurrence text editing."""
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
records=[]
for case,count,selected in [('unique',2,0),('first',3,0),('later',3,2)]:
    path=args.corpus/f'text-{case}-filled.pdf'
    digest=hashlib.sha256(path.read_bytes()).hexdigest()
    pdf=PdfReader(path)
    fields=pdf.trailer['/Root']['/AcroForm']['/Fields']
    assert len(pdf.pages)==count and len(fields)==1
    field=fields[0].get_object()
    assert field['/T']=='shared' and field['/V']=='Linked after editing'
    assert len(field['/Kids'])==count
    for page in pdf.pages:
        widgets=[a.get_object() for a in page['/Annots'] if a.get_object()['/Subtype']=='/Widget']
        assert len(widgets)==1 and widgets[0].raw_get('/Parent')==fields[0]
    with fitz.open(path) as actual:
        assert 'Next content' in actual[selected].get_text()
        for page in actual:
            widgets=list(page.widgets())
            assert len(widgets)==1 and widgets[0].field_value=='Linked after editing'
    assert hashlib.sha256(path.read_bytes()).hexdigest()==digest
    records.append(dict(case=case,pages=count,sha256=digest))
with args.output.open('x',encoding='utf-8') as output:
    json.dump(dict(passed=True,cases=records,mupdf=fitz.VersionBind),output,indent=2)
    output.write('\n')
print(json.dumps(dict(passed=True,cases=3,pages=8)))
