"""Independent development check using PyMuPDF and pypdf (not app dependencies)."""
import argparse
import json
from pathlib import Path
import fitz
from pypdf import PdfReader
from pypdf.generic import IndirectObject
parser=argparse.ArgumentParser()
parser.add_argument('pdf',type=Path)
parser.add_argument('--comment')
parser.add_argument('--highlight-note')
parser.add_argument('--expect-none',action='store_true')
args=parser.parse_args()
reader=PdfReader(args.pdf)
for page in reader.pages:
    for annotation in page.get('/Annots',[]):
        assert isinstance(annotation,IndirectObject), 'Annotation dictionaries must be indirect for reader interoperability'
doc=fitz.open(args.pdf)
annotations=[]
for index,page in enumerate(doc):
    for annot in page.annots() or []:
        annotations.append({'page':index+1,'type':annot.type[1],'contents':annot.info.get('content'), 'rect':list(annot.rect),'vertices':annot.vertices})
    if index==0:
        with_annotations=page.get_pixmap(matrix=fitz.Matrix(1.5,1.5),annots=True)
        without=page.get_pixmap(matrix=fitz.Matrix(1.5,1.5),annots=False)
        if not args.expect_none:
            assert with_annotations.samples!=without.samples, 'Annotations are not visible in MuPDF render'
        with_annotations.save(str(args.pdf.with_suffix('.mupdf.png')))
if args.comment:
    assert any(a['type']=='Text' and a['contents']==args.comment for a in annotations),annotations
if args.highlight_note:
    assert any(a['type']=='Highlight' and a['contents']==args.highlight_note for a in annotations),annotations
if args.expect_none:
    assert not any(a['type'] in ['Text','Highlight'] for a in annotations),annotations
result={'passed':True,'reader':'PyMuPDF/MuPDF and pypdf','file':str(args.pdf),'annotations':annotations}
args.pdf.with_suffix('.inspection.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(result,ensure_ascii=False,indent=2))