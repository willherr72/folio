"""Verify native paragraph-movement fixtures with readers independent of Folio."""
import hashlib
import json
from pathlib import Path

import fitz
from PIL import Image, ImageChops, ImageDraw
from pypdf import PdfReader

root = Path('artifacts/paragraph-position-native')
report = []
def span(page, text):
    return next(span for block in page.get_text('dict')['blocks'] if 'lines' in block for line in block['lines'] for span in line['spans'] if span['text'] == text)

for prefix in ['regular', 'cropped']:
    metadata = json.loads((root / f'{prefix}.json').read_text())
    source_path = root / f'{prefix}-original.pdf'
    source_hash = hashlib.sha256(source_path.read_bytes()).hexdigest()
    for rotation in [0, 90]:
        source = fitz.open(source_path)
        path = root / f'{prefix}-moved-{rotation}.pdf'
        output = fitz.open(path)
        assert output[0].rotation == rotation
        for text in [output[0].get_text(), PdfReader(path).pages[0].extract_text()]:
            assert text.count('Moved this paragraph.') == 1 and text.count('Neighbor stays.') == 1 and 'Original words.' not in text, text
        before = span(source[0], 'Neighbor stays.')
        after = span(output[0], 'Neighbor stays.')
        assert before['bbox'] == after['bbox'] and before['origin'] == after['origin']
        moved = span(output[0], 'Moved this paragraph.')
        assert abs(moved['origin'][0] - metadata['new']['x']) < 0.1, moved
        assert abs(moved['origin'][1] - metadata['new']['y'] - moved['size']) < 0.1, moved
        source[0].set_rotation(rotation)
        a, b = [page.get_pixmap(matrix=fitz.Matrix(2, 2), colorspace=fitz.csRGB) for page in [source[0], output[0]]]
        assert (a.width, a.height) == (b.width, b.height)
        difference = ImageChops.difference(Image.frombytes('RGB', (a.width, a.height), a.samples), Image.frombytes('RGB', (b.width, b.height), b.samples))
        assert difference.getbbox(), 'The paragraph must actually move/change'
        draw = ImageDraw.Draw(difference)
        for key in ['old', 'new']:
            bounds = metadata[key]
            rect = fitz.Rect(bounds['x'], bounds['y'], bounds['x'] + bounds['width'], bounds['y'] + bounds['height']) * source[0].rotation_matrix
            draw.rectangle((int(rect.x0 * 2) - 3, int(rect.y0 * 2) - 3, int(rect.x1 * 2) + 3, int(rect.y1 * 2) + 3), fill=(0, 0, 0))
        assert difference.getbbox() is None, (prefix, rotation, difference.getbbox())
        assert hashlib.sha256(source_path.read_bytes()).hexdigest() == source_hash
        report.append({'fixture': prefix, 'rotation': rotation, 'text_readers': ['MuPDF', 'pypdf'], 'baseline_origin': moved['origin'], 'neighbor_geometry_unchanged': True, 'pixels_unchanged_outside_old_and_new_areas': True, 'source_sha256': source_hash})
(root / 'independent-results.json').write_text(json.dumps(report, indent=2))
print(json.dumps(report))
