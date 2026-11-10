"""Generate an independent PDF fixture or verify the desktop comment round trip."""
from pathlib import Path
import hashlib
import json
import os
import sys
import fitz
import pypdf
from pypdf.generic import DictionaryObject, NameObject, NumberObject, TextStringObject, ArrayObject, FloatObject

root = Path(os.environ.get('FOLIO_COMMENT_ACCEPTANCE_ROOT', 'artifacts/comment-fidelity-desktop'))
root.mkdir(exist_ok=True)
source = root / 'Comment fidelity.pdf'
output = root / 'Folio edited.pdf'

if '--generate' in sys.argv:
    assert not source.exists(), 'Keep the existing acceptance source unchanged'
    writer = pypdf.PdfWriter(clone_from='artifacts/native-comments-desktop/Native comments.pdf')
    page = writer.pages[1]
    original = next(a.get_object() for a in page['/Annots'] if a.get_object().get('/Subtype') == '/Text')
    original[NameObject('/F')] = NumberObject(4)
    for text, rect in [('Color note', [180, 200, 204, 224]), ('Remove this note', [220, 140, 244, 164])]:
        note = DictionaryObject({NameObject('/Type'): NameObject('/Annot'), NameObject('/Subtype'): NameObject('/Text'),
            NameObject('/F'): NumberObject(4), NameObject('/Contents'): TextStringObject(text),
            NameObject('/T'): TextStringObject('External reviewer'), NameObject('/Name'): NameObject('/Comment'),
            NameObject('/Rect'): ArrayObject([NumberObject(n) for n in rect]),
            NameObject('/C'): ArrayObject([FloatObject(1), FloatObject(.8), FloatObject(0)]),
            NameObject('/CreationDate'): TextStringObject('D:20261022090000'), NameObject('/M'): TextStringObject('D:20261023090000')})
        ref = writer._add_object(note)
        page['/Annots'].append(ref)
        if text == 'Remove this note':
            popup = writer._add_object(DictionaryObject({NameObject('/Type'): NameObject('/Annot'), NameObject('/Subtype'): NameObject('/Popup'),
                NameObject('/Rect'): ArrayObject([NumberObject(n) for n in [250, 120, 380, 220]]), NameObject('/Parent'): ref}))
            note[NameObject('/Popup')] = popup
            page['/Annots'].append(popup)
    writer.write(source)
    print('Generated', source)
    sys.exit(0)

result = json.loads((root / 'results.json').read_text(encoding='utf-8'))
assert result['passed']
assert hashlib.sha256(source.read_bytes()).hexdigest() == result['originalSha256']
before, after = pypdf.PdfReader(source), pypdf.PdfReader(output)
def notes(page):
    return [a.get_object() for a in page.get('/Annots', []) if a.get_object().get('/Subtype') == '/Text']
assert [len(notes(p)) for p in before.pages] == [1, 3]
assert [len(notes(p)) for p in after.pages] == [1, 2]
old, new = notes(before.pages[1])[0], notes(after.pages[1])[0]
assert new['/Contents'] == 'Moved café 日本語'
for key in ['/T', '/Subj', '/Name', '/NM', '/F', '/C', '/CA', '/CreationDate', '/M']:
    assert old.get(key) == new.get(key), (key, old.get(key), new.get(key))
assert old['/AP']['/N'].get_object().get_data() == new['/AP']['/N'].get_object().get_data()
assert new['/Popup']['/Parent'].get_object()['/Contents'] == new['/Contents']
assert old['/Popup']['/Rect'] == new['/Popup']['/Rect']
colored = next(n for n in notes(after.pages[1]) if n['/Contents'] == 'Recolored note')
assert [round(float(n) * 255) for n in colored['/C']] == [18, 52, 86]
assert colored['/T'] == 'External reviewer'
assert colored['/CreationDate'] == 'D:20261022090000'
assert sum(a.get_object().get('/Subtype') == '/Popup' for a in after.pages[1]['/Annots']) == 1
assert all(n['/Contents'] != 'Remove this note' for p in after.pages for n in notes(p))
assert notes(before.pages[0])[0]['/Contents'] == notes(after.pages[0])[0]['/Contents'] == 'Locked reference note'
checks = ['standard-note metadata and custom AP preserved', 'popup parent and popup rectangle preserved',
          'plain-icon color and author preserved', 'deleted note and its popup absent', 'locked note preserved']
with fitz.open(source) as a, fitz.open(output) as b:
    assert a[1].rotation == b[1].rotation
    note = next(n for n in b[1].annots() if n.info['content'] == 'Moved café 日本語')
    # PDFium may normalize rectangle coordinates during export; compare displayed coordinates.
    bounds = fitz.Rect([float(v) for v in new['/Rect']]) * b[1].transformation_matrix * b[1].rotation_matrix
    assert abs(bounds.x0 - 80) < .02 and abs(bounds.y0 - 100) < .02, bounds
    for index in range(2):
        assert a[index].get_text() == b[index].get_text()
        for scale in [1, 2]:
            p, q = [doc[index].get_pixmap(matrix=fitz.Matrix(scale, scale), alpha=False, annots=False) for doc in [a, b]]
            assert (p.width, p.height, p.samples) == (q.width, q.height, q.samples), (index, scale)
    checks.append('underlying page pixels and text unchanged at 72 and 144 dpi')
(root / 'independent-verification.json').write_text(json.dumps({'passed': True, 'checks': checks}, indent=2), encoding='utf-8')
print('PASS: ' + '; '.join(checks))
