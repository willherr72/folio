"""Verify the real Windows edit of a previously saved, rotated PDF."""
import hashlib
import json
import os
from pathlib import Path
import fitz
from PIL import Image, ImageChops, ImageDraw
from pypdf import PdfReader

root = Path(os.environ.get('FOLIO_PARAGRAPH_ACCEPTANCE_ROOT', 'artifacts/rotated-paragraph-desktop'))
acceptance = json.loads((root / 'results.json').read_text())
assert acceptance['passed']
original_path = root / 'Rotated paragraph.pdf'
assert hashlib.sha256(original_path.read_bytes()).hexdigest() == acceptance['sourceHash']
before = fitz.open(original_path)
output_path = root / 'Edited rotated paragraph.pdf'
after = fitz.open(output_path)
assert len(before) == len(after) == 1 and before[0].rotation == after[0].rotation == 90
assert before[0].cropbox == after[0].cropbox and before[0].mediabox == after[0].mediabox
for text in [after[0].get_text(), PdfReader(output_path).pages[0].extract_text()]:
    for expected in ['Reopened rotated paragraph.', 'Still editable.', 'Neighbor stays.']:
        assert text.count(expected) == 1, text
    assert 'Positioned paragraph.' not in text

def span(page, text):
    return next((s, line['dir']) for b in page.get_text('dict')['blocks'] if 'lines' in b
                for line in b['lines'] for s in line['spans'] if s['text'] == text)

assert span(before[0], 'Neighbor stays.') == span(after[0], 'Neighbor stays.')
original, original_direction = span(before[0], 'Positioned paragraph.')
for index, text in enumerate(['Reopened rotated paragraph.', 'Still editable.']):
    moved, direction = span(after[0], text)
    assert direction == original_direction == (1.0, 0.0)
    assert abs(moved['origin'][0] - 130) < 0.1
    assert abs(moved['origin'][1] - 217 - index * 14.4) < 0.1
    for key in ['font', 'size', 'color']:
        assert moved[key] == original[key]
assert [a.info['content'] for a in after[0].annots()] == ['Keep position note']
pixmaps = [page.get_pixmap(matrix=fitz.Matrix(2, 2), colorspace=fitz.csRGB)
           for page in [before[0], after[0]]]
images = [Image.frombytes('RGB', (pix.width, pix.height), pix.samples) for pix in pixmaps]
assert images[0].size == images[1].size
difference = ImageChops.difference(*images)
assert difference.getbbox()
draw = ImageDraw.Draw(difference)
for key in ['old', 'new']:
    r = acceptance[key]
    draw.rectangle((int(r['x'] * 2) - 3, int(r['y'] * 2) - 3,
                    int((r['x'] + r['width']) * 2) + 3, int((r['y'] + r['height']) * 2) + 3), fill=(0, 0, 0))
assert difference.getbbox() is None, difference.getbbox()
report = dict(passed=True, rotation=90, first_baseline=[130, 217], text_readers=['MuPDF', 'pypdf'],
              font_and_direction_unchanged=True, neighbor_geometry_unchanged=True,
              comment_preserved=True, outside_pixels_unchanged=True, source_hash_unchanged=True)
(root / 'independent-results.json').write_text(json.dumps(report, indent=2))
print(json.dumps(report))
