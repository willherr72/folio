"""Independent oracle for the single-source occurrence replacement feasibility probe."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import fitz
from pypdf import PdfReader

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--input',required=True,type=Path)
parser.add_argument('--output',required=True,type=Path)
args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=False)
spec=importlib.util.spec_from_file_location('replacement_probe',Path(__file__).with_name('probe-page-replacement.py'))
probe=importlib.util.module_from_spec(spec);spec.loader.exec_module(probe)
original=Path('artifacts/page-replacement-navigation-20261024/target.pdf')
incoming=original.with_name('incoming.pdf')
paths=[original,incoming,*args.input.glob('*.pdf')]
digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
hashes={str(p):digest(p) for p in paths}
reference=args.output/'reference.pdf';probe.compose(original,incoming,reference,True)
records=[]
with fitz.open(original) as before,fitz.open(reference) as expected:
    for selected in [0,2]:
        path=args.input/f'selected-{selected}.pdf';pdf=PdfReader(path)
        assert len(pdf.pages)==3
        fields=pdf.get_fields();assert list(fields)==['retained']
        root=pdf.trailer['/Root']['/AcroForm']['/Fields'];assert len(root)==1
        field=root[0];assert len(field.get_object()['/Kids'])==3
        ids=[p.indirect_reference for p in pdf.pages]
        for index,page in enumerate(pdf.pages):
            found=[]
            for ref in page['/Annots']:
                annotation=ref.get_object()
                if annotation['/Subtype']=='/Widget':assert annotation.raw_get('/Parent')==field
                if annotation['/Subtype']=='/Link':
                    dest=annotation.get('/Dest')
                    action=annotation.get('/A')
                    if dest is None and action is not None and action.get_object().get('/S')=='/GoTo':dest=action.get_object()['/D']
                    if dest is not None:
                        assert dest[0] in ids
                        found.append(ids.index(dest[0]))
            assert found==([0,0] if index==1 else [1,1]),(selected,index,found)
        with fitz.open(path) as actual:
            for index,old in [(0,0),(1,1),(2,0)]:
                wanted=expected[0] if index==selected else before[old]
                assert actual[index].get_text()==wanted.get_text()
                for scale in [1,2]:
                    a=actual[index].get_pixmap(matrix=fitz.Matrix(scale,scale))
                    b=wanted.get_pixmap(matrix=fitz.Matrix(scale,scale))
                    assert (a.width,a.height,a.samples)==(b.width,b.height,b.samples),(selected,index,scale)
        filled=PdfReader(args.input/f'selected-{selected}-filled.pdf')
        assert list(filled.get_fields())==['retained']
        assert str(filled.get_fields()['retained']['/V'])=='Shared after replacement'
        field=filled.trailer['/Root']['/AcroForm']['/Fields'][0]
        for page in filled.pages:
            widgets=[a.get_object() for a in page['/Annots'] if a.get_object()['/Subtype']=='/Widget']
            assert len(widgets)==1 and widgets[0].raw_get('/Parent')==field
        records.append(dict(selected=selected,passed=True,sharedWidgetCount=3,localTargets=[[1,1],[0,0],[1,1]]))
assert hashes=={str(p):digest(p) for p in paths}
report=dict(passed=True,feasibilityOnly=True,cases=records,sha256=hashes,mupdf=fitz.VersionBind)
(args.output/'result.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps(dict(passed=True,cases=len(records),feasibilityOnly=True)))
