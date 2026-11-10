"""Independent verification of outputs from smoke-ocr-portable.mjs."""
import hashlib
import json
import os
from pathlib import Path

import fitz
from pypdf import PdfReader

root = Path(os.environ.get('FOLIO_OCR_ACCEPTANCE_ROOT', 'artifacts/pdf-ocr-portable'))
acceptance = json.loads((root / 'results.json').read_text())
assert acceptance['passed']
source = fitz.open(root / 'Scan.pdf')
report = {'outputs': [], 'source_hashes_unchanged': True}
for filename in ['Scan.pdf', 'Append.pdf']:
    assert hashlib.sha256((root / filename).read_bytes()).hexdigest() == acceptance['sourceHash']
for filename, mapping in [('Single searchable.pdf', [0, 1]), ('Portable mixed.pdf', [0, 0, 1, 0, 1])]:
    document = fitz.open(root / filename)
    reader = PdfReader(root / filename)
    assert len(document) == len(mapping)
    for index, original_index in enumerate(mapping):
        expected = source[original_index].get_pixmap(matrix=fitz.Matrix(2, 2))
        actual = document[index].get_pixmap(matrix=fitz.Matrix(2, 2))
        assert (expected.width, expected.height, expected.samples) == (actual.width, actual.height, actual.samples), (filename, index, 'pixels')
        for text in [document[index].get_text(), reader.pages[index].extract_text()]:
            assert text.count('Native header') == 1, (filename, index, text)
            marker = ('CheckpointInvoice' if index == 0 else None) if len(mapping) == 2 else ('PortableInvoice' if index < 2 else 'Invoice' if original_index == 0 else 'Second scanned page')
            if marker:
                assert text.count(marker) == 1, (filename, index, text)
            elif index == 1:
                assert 'Second scanned page' not in text, text
        if original_index == 0:
            assert 'Keep this note' in [a.info['content'] for a in document[index].annots()], (filename, index, 'comment')
    expected_bookmarks = [[1, 'Invoice', 1], [1, 'Second page', 2]] if len(mapping) == 2 else [[1, 'Invoice', 1], [1, 'Second page', 3], [1, 'Invoice', 4], [1, 'Second page', 5]]
    assert document.get_toc() == expected_bookmarks, document.get_toc()
    report['outputs'].append({'file': filename, 'pages': len(mapping), 'pixels_identical': True, 'native_text_once': True, 'ocr_text_readers': ['MuPDF', 'pypdf'], 'comments_preserved': True, 'bookmarks': document.get_toc()})
(root / 'independent-results.json').write_text(json.dumps(report, indent=2))
print(json.dumps(report))
