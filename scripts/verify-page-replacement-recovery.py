"""Compare the restored workspace's saved PDF with the pre-restart replacement."""
import argparse,json
from pathlib import Path
import fitz
from pypdf import PdfReader
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--before',type=Path,required=True)
parser.add_argument('--after',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
for path in [args.before,args.after]:
    pdf=PdfReader(path);assert len(pdf.pages)==3
    fields=pdf.trailer['/Root']['/AcroForm']['/Fields'];assert len(fields)==1 and len(fields[0].get_object()['/Kids'])==3
    ids=[p.indirect_reference for p in pdf.pages]
    assert pdf.trailer['/Root']['/Outlines']['/First']['/Dest'][0]==ids[0]
    for i,page in enumerate(pdf.pages):
        links=[a.get_object() for a in page['/Annots'] if a.get_object()['/Subtype']=='/Link'];assert len(links)==3
        for link in links:
            name=str(link['/NM']);dest=link['/A']['/D'] if name=='named' else link['/Dest']
            assert dest[0]==ids[i if name=='self' else 0 if i==2 else 2]
with fitz.open(args.before) as before,fitz.open(args.after) as after:
    for a,b in zip(before,after):
        assert a.get_text()==b.get_text() and a.rotation==b.rotation and a.cropbox==b.cropbox
        for scale in [1,2]:
            x=a.get_pixmap(matrix=fitz.Matrix(scale,scale),annots=True);y=b.get_pixmap(matrix=fitz.Matrix(scale,scale),annots=True)
            assert (x.width,x.height,x.samples)==(y.width,y.height,y.samples)
report=dict(passed=True,pages=3,exactText=True,exactPixelsAtScales=[1,2],frames=True,sharedField=True,links=True,bookmarks=True)
with args.output.open('x',encoding='utf-8') as stream:json.dump(report,stream,indent=2);stream.write('\n')
print(json.dumps(report))
