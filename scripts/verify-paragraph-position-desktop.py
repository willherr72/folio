"""Verify the saved PDF from the real Windows paragraph-positioning workflow."""
import hashlib
import json
import os
from pathlib import Path
import fitz
from PIL import Image, ImageChops, ImageDraw
from pypdf import PdfReader

root = Path(os.environ.get('FOLIO_PARAGRAPH_ACCEPTANCE_ROOT', 'artifacts/paragraph-position-desktop'))
acceptance = json.loads((root / 'results.json').read_text())
assert acceptance['passed']
assert hashlib.sha256((root / 'Original paragraph.pdf').read_bytes()).hexdigest() == acceptance['sourceHash']
before = fitz.open(root / 'Before move.pdf')
after = fitz.open(root / 'Positioned paragraph.pdf')
assert len(before) == len(after) == 1 and after[0].rotation == 90
for text in [after[0].get_text(), PdfReader(root / 'Positioned paragraph.pdf').pages[0].extract_text()]:
    assert text.count('Positioned paragraph.') == 1 and text.count('Neighbor stays.') == 1 and 'Original words.' not in text
def span(page, text):
    return next(s for b in page.get_text('dict')['blocks'] if 'lines' in b for line in b['lines'] for s in line['spans'] if s['text'] == text)
original_neighbor, final_neighbor = span(before[0], 'Neighbor stays.'), span(after[0], 'Neighbor stays.')
assert original_neighbor['origin'] == final_neighbor['origin'] and original_neighbor['bbox'] == final_neighbor['bbox']
moved = span(after[0], 'Positioned paragraph.')
assert abs(moved['origin'][0] - acceptance['new']['x']) < 0.1
assert abs(moved['origin'][1] - acceptance['new']['y'] - moved['size']) < 0.1
assert [a.info['content'] for a in after[0].annots()] == ['Keep position note']
before[0].set_rotation(90)
a, b = [page.get_pixmap(matrix=fitz.Matrix(2, 2), colorspace=fitz.csRGB) for page in [before[0], after[0]]]
assert (a.width, a.height) == (b.width, b.height)
difference = ImageChops.difference(Image.frombytes('RGB', (a.width, a.height), a.samples), Image.frombytes('RGB', (b.width, b.height), b.samples))
assert difference.getbbox()
draw = ImageDraw.Draw(difference)
for key in ['old', 'new']:
    bounds = acceptance[key]
    r = fitz.Rect(bounds['x'], bounds['y'], bounds['x'] + bounds['width'], bounds['y'] + bounds['height']) * before[0].rotation_matrix
    draw.rectangle((int(r.x0 * 2) - 3, int(r.y0 * 2) - 3, int(r.x1 * 2) + 3, int(r.y1 * 2) + 3), fill=(0, 0, 0))
assert difference.getbbox() is None, difference.getbbox()
report = {'passed': True, 'rotation': 90, 'baseline_origin': moved['origin'], 'text_readers': ['MuPDF', 'pypdf'], 'pixels_unchanged_outside_old_and_new_areas': True, 'neighbor_geometry_unchanged': True, 'comment_preserved': True, 'source_hash_unchanged': True}
(root / 'independent-results.json').write_text(json.dumps(report, indent=2))
print(json.dumps(report))
