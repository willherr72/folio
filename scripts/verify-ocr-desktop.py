"""Independent readers verify PDFs produced by the real OCR desktop workflow."""
import hashlib, json
from pathlib import Path
import fitz
from pypdf import PdfReader

root = Path('artifacts/pdf-ocr-desktop')
baseline = fitz.open(root / 'Before OCR.pdf')
report = {'outputs': [], 'originals_unchanged': True}
for name, marker in [('Searchable scan.pdf', 'CorrectedInvoice'), ('Corrected scan.pdf', 'ReopenedInvoice')]:
    doc = fitz.open(root / name)
    other = PdfReader(root / name)
    assert len(doc) == len(baseline) == 2
    for i in range(2):
        assert baseline[i].get_pixmap(matrix=fitz.Matrix(2, 2)).samples == doc[i].get_pixmap(matrix=fitz.Matrix(2, 2)).samples
        for text in [doc[i].get_text(), other.pages[i].extract_text()]:
            assert text.count('Native header') == 1, text
            assert (marker if i == 0 else 'Second scanned page') in text, text
    assert doc.get_toc() == baseline.get_toc()
    notes = [a.info['content'] for a in doc[0].annots()]
    assert 'Keep this note' in notes and 'Review OCR result' in notes, notes
    assert not other.is_encrypted
    report['outputs'].append({'file': name, 'pages': 2, 'pixels_identical': True, 'text_readers': ['MuPDF', 'pypdf'], 'comments': notes, 'bookmarks': doc.get_toc()})
for path, expected in json.loads((root / 'original-hashes.json').read_text(encoding='utf-8-sig')).items():
    assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == expected, path
for name, marker in [('Protected searchable.pdf','ProtectedInvoice'),('Recovered protected.pdf','RecoveryInvoice')]:
    protected_path = root / name
    if not protected_path.exists():
        continue
    original = fitz.open(root / 'Protected scan.pdf')
    assert original.authenticate('reader')
    protected = fitz.open(protected_path)
    assert protected.needs_pass and protected.authenticate('reader')
    assert protected.permissions == original.permissions
    assert len(protected) == len(original)
    for i in range(len(protected)):
        assert original[i].get_pixmap(matrix=fitz.Matrix(2, 2)).samples == protected[i].get_pixmap(matrix=fitz.Matrix(2, 2)).samples
    assert marker in protected[0].get_text()
    assert protected[0].get_text().count('Native header') == 1
    reader = PdfReader(protected_path)
    source = PdfReader(root / 'Protected scan.pdf')
    assert reader.is_encrypted and reader.decrypt('reader') and source.decrypt('reader')
    assert reader.trailer['/Encrypt']['/P'] == source.trailer['/Encrypt']['/P']
    assert reader.trailer['/Encrypt']['/R'] == source.trailer['/Encrypt']['/R']
    assert marker in reader.pages[0].extract_text()
    assert PdfReader(protected_path).decrypt('owner') == 2
    report[name] = {'pixels_identical': True, 'reader_and_owner_credentials': True, 'permissions_preserved': True, 'searchable_text': True}
def character_boxes(page, marker):
    chars = [c for block in page.get_text('rawdict')['blocks'] if block['type'] == 0 for line in block['lines'] for span in line['spans'] for c in span['chars']]
    text = ''.join(c['c'] for c in chars)
    start = text.index(marker)
    return [c['bbox'] for c in chars[start:start+len(marker)]]
original = fitz.open(root / 'Corrected scan.pdf')
original[0].set_rotation(90)  # PDF rotation respects annotations' NoRotate flags.
for name, marker in [('Rotated searchable.pdf','RotatedInvoice!'),('Rotated reopened.pdf','ReopenedInvoice')]:
    path = root / name
    if not path.exists():
        continue
    rotated = fitz.open(path)
    assert rotated[0].rotation == 90
    assert original[0].get_pixmap(matrix=fitz.Matrix(2, 2)).samples == rotated[0].get_pixmap(matrix=fitz.Matrix(2, 2)).samples
    a,b = character_boxes(original[0],'ReopenedInvoice'),character_boxes(rotated[0],marker)
    assert len(a) == len(b)
    error = max(abs(x-y) for ra,rb in zip(a,b) for x,y in zip(ra,rb))
    assert error < 0.1, (name,error)
    assert marker in PdfReader(path).pages[0].extract_text()
    report[name] = {'rotated_pixels_identical': True, 'character_box_maximum_error_pt': error, 'reader_text': True}
(root / 'independent-results.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
print(json.dumps(report))
