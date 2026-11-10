"""Independent saved desktop text-edit/undo pixels and linked-field verification."""
import argparse,json
from pathlib import Path
import fitz
from pypdf import PdfReader,PdfWriter
from pypdf.generic import DecodedStreamObject,NameObject
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--run',type=Path,required=True)
parser.add_argument('--output',type=Path)
args=parser.parse_args();root=args.run/'workflow';out=args.output or args.run/'independent';out.mkdir(exist_ok=False)
baseline=PdfReader(root/'baseline.pdf')
files=['baseline.pdf','first-edit.pdf','undo-reedit.pdf','undone.pdf','edited.pdf','filled.pdf']
for name in files:
    pdf=PdfReader(root/name);assert len(pdf.pages)==3
    fields=pdf.trailer['/Root']['/AcroForm']['/Fields'];assert len(fields)==1
    field=fields[0].get_object();assert len(field['/Kids'])==3
    assert field['/V']==('Desktop linked' if name=='filled.pdf' else 'Keep field')
    ids=[p.indirect_reference for p in pdf.pages]
    assert pdf.trailer['/Root']['/Outlines']['/First']['/Dest'][0]==ids[0]
    for i,page in enumerate(pdf.pages):
        assert page.mediabox==baseline.pages[i].mediabox and page.cropbox==baseline.pages[i].cropbox and page.rotation==baseline.pages[i].rotation
        widgets=[a.get_object() for a in page['/Annots'] if a.get_object()['/Subtype']=='/Widget'];assert len(widgets)==1 and widgets[0].raw_get('/Parent')==fields[0]
        links=[a.get_object() for a in page['/Annots'] if a.get_object()['/Subtype']=='/Link'];assert len(links)==3
        for link in links:
            name=str(link['/NM']);dest=link['/A']['/D'] if name=='named' else link['/Dest']
            assert dest[0]==ids[i if name=='self' else 0 if i==2 else 2]
for text in ['New content','Next content']:
    writer=PdfWriter(clone_from=root/'original.pdf');page=writer.pages[0]
    data=page.get_contents().get_data();assert data.count(b'(Old content)')==1
    stream=DecodedStreamObject();stream.set_data(data.replace(b'(Old content)',f'({text})'.encode()))
    page[NameObject('/Contents')]=writer._add_object(stream)
    page.cropbox=baseline.pages[1].cropbox;page.rotation=baseline.pages[1].rotation
    writer.write(out/f'{text.split()[0]}-reference.pdf')
with fitz.open(root/'baseline.pdf') as base,fitz.open(out/'New-reference.pdf') as first,fitz.open(out/'Next-reference.pdf') as second:
    for name in ['first-edit.pdf','undo-reedit.pdf','undone.pdf','edited.pdf']:
        with fitz.open(root/name) as actual:
            for i in range(3):
                expected=base[i] if i!=1 or name=='undone.pdf' else second[0] if name=='edited.pdf' else first[0]
                assert actual[i].get_text()==expected.get_text(),(name,i,'text')
                for scale in [1,2]:
                    a=actual[i].get_pixmap(matrix=fitz.Matrix(scale,scale),annots=True);b=expected.get_pixmap(matrix=fitz.Matrix(scale,scale),annots=True)
                    assert (a.width,a.height,a.samples)==(b.width,b.height,b.samples),(name,i,scale,'pixels')
with fitz.open(root/'filled.pdf') as filled,fitz.open(root/'edited.pdf') as edited:
    for a,b in zip(filled,edited):
        widgets=list(a.widgets());assert len(widgets)==1 and widgets[0].field_value=='Desktop linked'
        # MuPDF extraction includes widget appearances: expect the deliberate
        # field-value change, while requiring all surrounding text to match.
        before=b.get_text();assert before.count('Keep field\n')==1
        assert a.get_text()==before.replace('Keep field\n','Desktop linked\n')
report=dict(passed=True,savedDocuments=6,exactPixelDocuments=4,pagesPerDocument=3,exactPixelsAtScales=[1,2],sharedFields=True,filledValues=True,links=True,bookmarks=True,frames=True)
with (out/'result.json').open('x',encoding='utf-8') as output:json.dump(report,output,indent=2);output.write('\n')
print(json.dumps(report))
