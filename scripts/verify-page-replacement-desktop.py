"""Independent saved desktop replacement and undo checks, including native links/forms."""
import argparse,importlib.util,json
from pathlib import Path
import fitz
from pypdf import PdfReader,PdfWriter
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--run',type=Path,required=True)
args=parser.parse_args();root=args.run/'workflow';out=args.run/'independent';out.mkdir(exist_ok=False)
baseline=PdfReader(root/'baseline.pdf');actual=PdfReader(root/'replaced.pdf')
assert len(baseline.pages)==len(actual.pages)==3
for pdf in [baseline,actual]:
    fields=pdf.trailer['/Root']['/AcroForm']['/Fields'];assert len(fields)==1
    assert len(fields[0].get_object()['/Kids'])==3
    ids=[p.indirect_reference for p in pdf.pages]
    assert pdf.trailer['/Root']['/Outlines']['/First']['/Dest'][0]==ids[0]
    for index,page in enumerate(pdf.pages):
        links=[v.get_object() for v in page['/Annots'] if v.get_object()['/Subtype']=='/Link'];assert len(links)==3
        for link in links:
            name=str(link['/NM']);destination=link['/A']['/D'] if name=='named' else link['/Dest']
            assert destination[0]==ids[index if name=='self' else 0 if index==2 else 2]
for a,b in zip(baseline.pages,actual.pages):
    assert a.mediabox==b.mediabox and a.cropbox==b.cropbox and a.rotation==b.rotation
spec=importlib.util.spec_from_file_location('replacement_probe',Path(__file__).with_name('probe-page-replacement.py'))
probe=importlib.util.module_from_spec(spec);spec.loader.exec_module(probe)
probe.compose(root/'original.pdf',root/'incoming.pdf',out/'native-reference.pdf',True)
writer=PdfWriter(clone_from=out/'native-reference.pdf')
writer.pages[0].cropbox=baseline.pages[1].cropbox;writer.pages[0].rotation=baseline.pages[1].rotation
writer.write(out/'display-reference.pdf')
with fitz.open(root/'baseline.pdf') as base,fitz.open(root/'undone.pdf') as undone,fitz.open(root/'replaced.pdf') as final,fitz.open(out/'display-reference.pdf') as reference:
    for i in range(3):
        assert base[i].get_text()==undone[i].get_text()
        expected=reference[0] if i==1 else base[i]
        assert final[i].get_text()==expected.get_text(),(i,'text')
        for scale in [1,2]:
            pix=lambda page:page.get_pixmap(matrix=fitz.Matrix(scale,scale),annots=True)
            a,b,c,d=pix(base[i]),pix(undone[i]),pix(final[i]),pix(expected)
            assert (a.width,a.height,a.samples)==(b.width,b.height,b.samples),(i,'undo',scale)
            assert (c.width,c.height,c.samples)==(d.width,d.height,d.samples),(i,'replacement',scale)
report=dict(passed=True,pages=3,undo=True,exactPixelsAtScales=[1,2],links=True,sharedField=True,bookmarks=True,frames=True)
(out/'result.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8');print(json.dumps(report))
