"""Independent checks of the owned Windows renderer acceptance outputs."""
from pathlib import Path
import json
import runpy
import sys
import shutil
import fitz
from PIL import Image
from pypdf import PdfReader

root=Path(sys.argv[1])
helpers=runpy.run_path(str(Path(__file__).with_name('verify-form-text-flow.py')))
equal=helpers['equal_page']
checks=[]
dependent='--dependent' in sys.argv[2:]
if dependent:
    proof=root/'edited-proof';proof.mkdir(exist_ok=False)
    for prefix in ['standard-0','simple-0','cid-0']:
        shutil.copyfile(root/f'{prefix}-source.pdf',proof/f'{prefix}-source.pdf')
        shutil.copyfile(root/f'{prefix}-edited.pdf',proof/f'{prefix}-saved.pdf')
        with fitz.open(root/f'{prefix}-source.pdf') as source:
            for suffix in ['undo','restored']:
                with fitz.open(root/f'{prefix}-{suffix}.pdf') as output:
                    assert len(source)==len(output)==2
                    for a,b in zip(source,output):equal(a,b)
        checks.append(f'{prefix}: undo/restore match source pixels and glyph geometry')
    verify=runpy.run_path(str(Path(__file__).with_name('verify-form-dependent-text.py')))['verify']
    evidence=verify(proof,3,'Cafeolder')
    (proof/'independent-results.json').write_text(json.dumps(evidence,indent=2)+'\n')
    checks.append('Three longer replacements: PDF advances, fonts, untouched glyphs and pixels verified')
for prefix in ([] if dependent else ['standard-leading','simple-combined','cid-mirror']):
    with fitz.open(root/f'{prefix}-source.pdf') as source, fitz.open(root/f'{prefix}-reference.pdf') as reference:
        for suffix in ['undo','restored','edited']:
            with fitz.open(root/f'{prefix}-{suffix}.pdf') as output:
                assert len(output)==len(source)==2
                equal(output[0],reference[0] if suffix=='edited' else source[0])
                equal(output[1],source[1])
        checks.append(f'{prefix}: MuPDF pixels and glyph geometry; untouched second page')
fields=PdfReader(root/'filled.pdf').get_fields()
assert fields['full_name']['/TU']=='Full name'
assert fields['full_name']['/V']=='Renderer acceptance'
with fitz.open(root/'filled.pdf') as document, fitz.open(root/'Form.pdf') as original:
    pix=document[0].get_pixmap()
    assert len(set(pix.samples))>1
    assert pix.samples != original[0].get_pixmap().samples, 'Saved field value did not update its visible appearance'
checks.append('pypdf field value and changed nonblank MuPDF form appearance')
annotated=PdfReader(root/'annotated.pdf')
contents=[str(item.get_object().get('/Contents','')) for page in annotated.pages for item in page.get('/Annots',[])]
assert 'Renderer acceptance café' in contents and 'Locked reference note' in contents
checks.append('pypdf updated Unicode comment and preserved locked note')
result=json.loads((root/'results.json').read_text())
with fitz.open(root/'annotated.pdf') as document:
    for index,page in enumerate(document):
        with Image.open(Path(result['imageFolder'])/f'page-{index+1:04}.png') as image:
            expected=page.get_pixmap(matrix=fitz.Matrix(150/72,150/72))
            assert abs(image.width-expected.width)<=1 and abs(image.height-expected.height)<=1
            assert any(low<high for low,high in image.convert('RGB').getextrema())
checks.append('PNG dimensions agree with independent 150dpi page dimensions; nonblank output')
report=dict(passed=True,checks=checks,limits=['PNG pixels are not asserted equal across renderers.','Physical printer output is not tested.'])
with (root/'independent-results.json').open('x',encoding='utf-8') as output:
    json.dump(report,output,indent=2)
print(json.dumps(report))
