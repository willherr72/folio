"""Independent readers verify nested form edits, shared copies and font bytes."""
import hashlib,json,os
from pathlib import Path
import fitz
from pypdf import PdfReader

root=Path(os.environ.get('FOLIO_EMBEDDED_FORM_ARTIFACTS','artifacts/embedded-form-fixed-fixtures'))
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def raster(page):return page.get_pixmap(matrix=fitz.Matrix(2,2),colorspace=fitz.csRGB,alpha=False).samples
def programs(doc):
    return {hashlib.sha256(doc.extract_font(font[0])[3]).hexdigest() for page in doc for font in page.get_fonts(full=True) if doc.extract_font(font[0])[3]}
def equal(first,second):
    with fitz.open(first) as a,fitz.open(second) as b:
        assert len(a)==len(b),(first,second,'pages')
        for pa,pb in zip(a,b):
            assert pa.rotation==pb.rotation and pa.rect==pb.rect
            assert raster(pa)==raster(pb),(first,second,'pixels')
            assert pa.get_text('rawdict')==pb.get_text('rawdict'),(first,second,'glyphs')
        assert programs(a)==programs(b),(first,second,'font programs')
    assert [p.extract_text() for p in PdfReader(first).pages]==[p.extract_text() for p in PdfReader(second).pages]

checks=[]
for kind in ['simple','cid']:
    for rotation in [0,90,180,270]:
        prefix=f'{kind}-{rotation}'
        original=root/f'{prefix}-original.pdf';reference=root/f'{prefix}-reference.pdf';edited=root/f'{prefix}-edited.pdf'
        equal(reference,edited)
        with fitz.open(original) as before,fitz.open(edited) as after:
            assert programs(before)==programs(after)
            assert raster(before[1])==raster(after[1])
            assert after[0].get_text().count('Café edit')==1
            assert after[0].get_text().count('Café old')==1
        checks.append(f'{prefix}: independent reference pixels/glyphs, selected-only Unicode, second page and original fonts')

desktop=os.environ.get('FOLIO_EMBEDDED_FORM_DESKTOP_ROOT')
if desktop:
    desktop=Path(desktop);result=json.loads((desktop/'results.json').read_text(encoding='utf-8'));assert result['passed']
    for kind in ['simple','cid']:
        original=desktop/f'{kind}-original.pdf';assert sha(original)==result['sources'][kind]
        for suffix in ['before','undo','restored']:equal(original,desktop/f'{kind}-{suffix}.pdf')
        equal(root/f'{kind}-0-reference.pdf',desktop/f'{kind}-edited.pdf')
        checks.append(f'{kind} Windows: original pixels/glyphs on initial save, undo, re-edit; edited reference pixels/glyphs/fonts')
report={'passed':True,'readers':['MuPDF','pypdf'],'checks':checks}
destination=(desktop or root)/'independent-results.json';destination.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps(report))
