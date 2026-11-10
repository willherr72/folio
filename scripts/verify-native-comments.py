"""Independent pypdf metadata and MuPDF raster checks for desktop comment edits."""
from pathlib import Path
import hashlib
import json
import fitz
import pypdf

root = Path('artifacts/native-comments-desktop')
results = json.loads((root / 'results.json').read_text(encoding='utf-8'))
assert results['passed'] and results['desktop']
source = root / 'Native comments.pdf'
output = root / 'Folio edited.pdf'
assert hashlib.sha256(source.read_bytes()).hexdigest() == results['originalSha256']
original, edited = pypdf.PdfReader(source), pypdf.PdfReader(output)
assert len(original.pages) == len(edited.pages) == 2
checks = []

def notes(page):
    return [value.get_object() for value in page.get('/Annots', [])
            if value.get_object().get('/Subtype') == '/Text']

for index, (before_page, after_page) in enumerate(zip(original.pages, edited.pages)):
    before, after = notes(before_page), notes(after_page)
    assert len(before) == len(after) == 1
    before, after = before[0], after[0]
    expected = 'Locked reference note' if index == 0 else 'Updated café 日本語\nSecond line'
    assert after['/Contents'] == expected
    for key in ['/Type', '/Subtype', '/F', '/Rect', '/Name', '/C', '/CA', '/T', '/Subj',
                '/NM', '/M', '/CreationDate', '/Open', '/RC', '/State', '/StateModel']:
        assert before.get(key) == after.get(key), (index, key, before.get(key), after.get(key))
    assert before['/AP']['/N'].get_object().get_data() == after['/AP']['/N'].get_object().get_data()
    if '/Popup' in before:
        a, b = before['/Popup'].get_object(), after['/Popup'].get_object()
        for key in ['/Subtype', '/Rect', '/Open', '/F']:
            assert a.get(key) == b.get(key), (index, 'popup', key)
        assert b['/Parent'].get_object()['/Contents'] == expected
    assert list(before_page.mediabox) == list(after_page.mediabox)
    assert before_page.get_contents().get_data() == after_page.get_contents().get_data()
    checks.append(f'page {index+1}: text, metadata, appearance bytes, popup and page content preserved')

with fitz.open(source) as before, fitz.open(output) as after:
    for index in range(2):
        assert before[index].get_text() == after[index].get_text()
        for scale in [1, 2]:
            a = before[index].get_pixmap(matrix=fitz.Matrix(scale, scale), alpha=False, annots=True)
            b = after[index].get_pixmap(matrix=fitz.Matrix(scale, scale), alpha=False, annots=True)
            assert (a.width, a.height, a.samples) == (b.width, b.height, b.samples), (index, scale, 'raster changed')
    checks.append('all page pixels identical at 72 and 144 dpi, including native comment icons')
(root / 'independent-verification.json').write_text(json.dumps({'passed': True, 'checks': checks}, indent=2), encoding='utf-8')
print('PASS independent comment PDF verification: ' + '; '.join(checks))
