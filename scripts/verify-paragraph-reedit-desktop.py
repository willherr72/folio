"""Independent output proof: re-editing without changes must leave every pixel intact."""
import hashlib,json,os
from pathlib import Path
import fitz
from pypdf import PdfReader
from PIL import Image,ImageChops,ImageDraw

root=Path(os.environ.get('FOLIO_PARAGRAPH_REEDIT_ROOT','artifacts/paragraph-reedit-20261023/desktop'))
result=json.loads((root/'results.json').read_text(encoding='utf-8'));assert result['passed']
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(root/'Original paragraph.pdf')==result['sourceHash']
assert sha(root/'Substitute.ttf')==result['fontHash']
def image(page):
    p=page.get_pixmap(matrix=fitz.Matrix(2,2),colorspace=fitz.csRGB,alpha=False)
    return Image.frombytes('RGB',(p.width,p.height),p.samples)
def notes(path):
    return [(list(a.get_object()['/Rect']),a.get_object().get('/Contents'),a.get_object().get('/F')) for a in PdfReader(path).pages[0].get('/Annots',[]) if a.get_object().get('/Subtype')=='/Text']
def normalize(text):return ' '.join(text.split())
files=['Before edit.pdf','First edit.pdf','Unchanged re-edit.pdf','Rotated substitute.pdf','Rotated unchanged re-edit.pdf']
docs={name:fitz.open(root/name) for name in files}
assert all(len(doc)==1 for doc in docs.values())
for first,second in [('First edit.pdf','Unchanged re-edit.pdf'),('Rotated substitute.pdf','Rotated unchanged re-edit.pdf')]:
    a,b=docs[first][0],docs[second][0]
    assert a.rotation==b.rotation and a.rect==b.rect
    assert ImageChops.difference(image(a),image(b)).getbbox() is None,(first,second)
    assert a.get_text('rawdict')==b.get_text('rawdict'),(first,second,'glyph geometry')
    assert PdfReader(root/first).pages[0].extract_text()==PdfReader(root/second).pages[0].extract_text()
before,edited=docs['Before edit.pdf'][0],docs['First edit.pdf'][0]
diff=ImageChops.difference(image(before),image(edited));assert diff.getbbox()
# Original text and the entire replacement area are inside this common rectangle.
ImageDraw.Draw(diff).rectangle((60*2-3,80*2-3,240*2+3,220*2+3),fill=(0,0,0))
assert diff.getbbox() is None,diff.getbbox()
original_notes=notes(root/'Before edit.pdf');assert original_notes
for name in files[1:]:
    assert notes(root/name)==original_notes,name
    assert 'Neighbor stays.' in docs[name][0].get_text()
    expected=result['secondText'] if name.startswith('Rotated') else result['firstText']
    for extracted in [docs[name][0].get_text(),PdfReader(root/name).pages[0].extract_text()]:
        assert normalize(expected) in normalize(extracted),(name,extracted)
    hints=PdfReader(root/name).pages[0]['/FolioParagraphs']
    raw=hints.original_bytes if hasattr(hints,'original_bytes') else bytes(hints)
    assert b'Alpha' not in raw and b'Caf' not in raw and b'Original' not in raw
assert docs['Rotated substitute.pdf'][0].rotation==90
fonts=docs['Rotated substitute.pdf'][0].get_fonts(full=True)
embedded=[docs['Rotated substitute.pdf'].extract_font(font[0])[3] for font in fonts]
assert any(hashlib.sha256(data).hexdigest()==result['fontHash'] for data in embedded if data)
if result.get('colorChecks'):
    def spans(page):
        return [s for block in page.get_text('dict')['blocks'] if 'lines' in block for line in block['lines'] for s in line['spans'] if s['text'].strip()]
    neighbor=[s for s in spans(before) if 'Neighbor' in s['text']]
    assert neighbor
    for name in files[1:]:
        selected=[s for s in spans(docs[name][0]) if 'Neighbor' not in s['text']]
        assert selected and all(s['color']==(0xA02BC3 if name.startswith('Rotated') else 0x1A80EF) for s in selected),(name,selected)
        assert [s for s in spans(docs[name][0]) if 'Neighbor' in s['text']]==neighbor
    undo=fitz.open(root/'Undo edit.pdf')
    assert ImageChops.difference(image(before),image(undo[0])).getbbox() is None
    assert before.get_text('rawdict')==undo[0].get_text('rawdict')
    assert notes(root/'Undo edit.pdf')==original_notes
report={'passed':True,'readers':['MuPDF','pypdf'],'bothNoOpEditsPixelAndGlyphIdentical':True,'outsideFirstEditAreaPixelsUnchanged':True,'nativeNoteAndNeighborPreserved':True,'unicodeAndEmbeddedFontPreserved':True,'metadataContainsNoDuplicateParagraphWords':True}
if result.get('colorChecks'):report.update({'explicitColorsVerified':True,'undoRestoresOriginalPixelsAndGlyphs':True,'neighborStyleAndGeometryUnchanged':True})
(root/'independent-results.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps(report))
