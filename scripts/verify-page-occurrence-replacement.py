"""Independent native occurrence replacement geometry, interaction and pixel oracle."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import fitz
from pypdf import PdfReader

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--corpus',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=False)
spec=importlib.util.spec_from_file_location('replacement_probe',Path(__file__).with_name('probe-page-replacement.py'))
probe=importlib.util.module_from_spec(spec);spec.loader.exec_module(probe)
digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
records=[]
for result in sorted(args.corpus.glob('r*-result.pdf')):
    label=result.name.removesuffix('-result.pdf');selected=int(label.rsplit('-s',1)[1])
    source=args.corpus/f'{label}-source.pdf';incoming=args.corpus/f'{label}-incoming.pdf'
    paths=[source,incoming,result];hashes={p.name:digest(p) for p in paths}
    reference=args.output/f'{label}-reference.pdf';probe.compose(source,incoming,reference,True)
    before,after=PdfReader(source),PdfReader(result)
    assert len(before.pages)==2 and len(after.pages)==3
    old_root,new_root=before.trailer['/Root'],after.trailer['/Root']
    for key in ['/Lang','/PageLayout']:
        assert new_root[key]==old_root[key],(label,key)
    assert new_root['/Metadata'].get_data()==old_root['/Metadata'].get_data()
    for index in [0,1]:
        assert before.pages[index].indirect_reference.idnum==after.pages[index].indirect_reference.idnum
    assert list(after.get_fields())==['shared']
    fields=new_root['/AcroForm']['/Fields'];assert len(fields)==1
    assert len(fields[0].get_object()['/Kids'])==3
    ids=[page.indirect_reference for page in after.pages]
    targets=[]
    for index,old in [(0,0),(1,1),(2,0)]:
        page=after.pages[index];original=before.pages[old]
        assert page.mediabox==original.mediabox and page.cropbox==original.cropbox
        assert page.rotation==original.rotation
        assert float(page.get('/UserUnit',1))==float(original.get('/UserUnit',1))
        expected_note=[a.get_object() for a in original['/Annots'] if a.get_object()['/Subtype']=='/Text'][0]
        seen={}
        for annotation in page['/Annots']:
            annotation=annotation.get_object()
            if annotation['/Subtype']=='/Widget':
                assert annotation.raw_get('/Parent')==fields[0]
            elif annotation['/Subtype']=='/Text':
                for key in ['/Rect','/Contents','/F']:
                    assert annotation[key]==expected_note[key]
            elif annotation['/Subtype']=='/Link':
                name=str(annotation['/NM'])
                dest=annotation['/A']['/D'] if name=='named' else annotation['/Dest']
                assert dest[0] in ids
                target=ids.index(dest[0]);seen[name]=target
                assert target==(index if name=='self' else 0 if index==1 else 1),(label,index,name,target)
        assert set(seen)=={'direct','named','self'};targets.append(seen)
    bookmark=new_root['/Outlines']['/First']
    assert bookmark['/Title']=='Retained bookmark' and bookmark['/Dest'][0]==ids[0]
    with fitz.open(source) as old,fitz.open(result) as actual,fitz.open(reference) as expected:
        for index,original in [(0,0),(1,1),(2,0)]:
            wanted=expected[0] if index==selected else old[original]
            assert actual[index].get_text()==wanted.get_text(),(label,index,'text')
            for scale in [1,2]:
                a=actual[index].get_pixmap(matrix=fitz.Matrix(scale,scale))
                b=wanted.get_pixmap(matrix=fitz.Matrix(scale,scale))
                assert (a.width,a.height,a.samples)==(b.width,b.height,b.samples),(label,index,scale)
    assert hashes=={p.name:digest(p) for p in paths}
    records.append(dict(label=label,selected=selected,passed=True,targets=targets,sha256=hashes))
assert len(records)==16,f'Expected sixteen cases, got {len(records)}'
report=dict(passed=True,cases=records,mupdf=fitz.VersionBind,
            checks='Native page IDs, catalog metadata/bookmarks, one shared field with three widgets, comments, direct/named/self links, raw frames, exact text/pixels at two scales, source integrity')
(args.output/'result.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps(dict(passed=True,cases=len(records),pages=len(records)*3)))
