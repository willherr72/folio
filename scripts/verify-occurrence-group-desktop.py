"""Independent saved grouped text, untouched-page pixels, undo and linked fields."""
import argparse,json,re
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
def same_pixels(a,b,label):
    assert a.get_text()==b.get_text(),(label,'text')
    for scale in [1,2]:
        ap=a.get_pixmap(matrix=fitz.Matrix(scale,scale),annots=True);bp=b.get_pixmap(matrix=fitz.Matrix(scale,scale),annots=True)
        assert (ap.width,ap.height,ap.samples)==(bp.width,bp.height,bp.samples),(label,scale,'pixels')
with fitz.open(root/'baseline.pdf') as base:
    for name in ['first-edit.pdf','undo-reedit.pdf','undone.pdf','edited.pdf']:
        with fitz.open(root/name) as actual:
            for i in range(3):
                if i!=1 or name=='undone.pdf':same_pixels(actual[i],base[i],(name,i))
                else:
                    replacement='Next text' if name=='edited.pdf' else 'New text'
                    expected=base[i].get_text().replace('Old words',replacement)
                    assert re.sub(r'\s+',' ',actual[i].get_text()).strip()==re.sub(r'\s+',' ',expected).strip(),(name,i,'selected text')
for before,after in [('first-edit.pdf','undo-reedit.pdf')]:
    with fitz.open(root/before) as a,fitz.open(root/after) as b:
        for i in range(3):same_pixels(a[i],b[i],(before,after,i))
reference=PdfWriter(clone_from='artifacts/occurrence-group-20261024/corpus-corrected/group-r0-u1-unique-edit-reference.pdf')
reference.pages[0].cropbox=baseline.pages[1].cropbox
reference.pages[0].rotation=baseline.pages[1].rotation
reference.write(out/'first-reference.pdf')
with fitz.open(out/'first-reference.pdf') as expected,fitz.open(root/'first-edit.pdf') as actual:
    same_pixels(actual[1],expected[0],'selected group versus native reference')
with fitz.open(root/'filled.pdf') as filled,fitz.open(root/'edited.pdf') as edited:
    for a,b in zip(filled,edited):
        widgets=list(a.widgets());assert len(widgets)==1 and widgets[0].field_value=='Desktop linked'
        # MuPDF extraction includes widget appearances: expect the deliberate
        # field-value change, while requiring all surrounding text to match.
        before=b.get_text();assert before.count('Keep field\n')==1
        assert a.get_text()==before.replace('Keep field\n','Desktop linked\n')
report=dict(passed=True,savedDocuments=6,selectedGroupReferenceExact=True,untouchedPagesExact=True,undoExact=True,pagesPerDocument=3,exactPixelsAtScales=[1,2],sharedFields=True,filledValues=True,links=True,bookmarks=True,frames=True,limit="Selected changed group text checked; layout quality is visually inspected, not compared to an independent layout implementation.")
with (out/'result.json').open('x',encoding='utf-8') as output:json.dump(report,output,indent=2);output.write('\n')
print(json.dumps(report))
