"""Independent rotated/cropped paragraph text, geometry and pixel acceptance."""
import hashlib
import json
from pathlib import Path

import fitz
from PIL import Image, ImageChops, ImageDraw
from pypdf import PdfReader

root = Path('artifacts/rotated-paragraph-native')
report = []

def spans(page):
    return [(span, line['dir']) for block in page.get_text('dict')['blocks']
            if 'lines' in block for line in block['lines'] for span in line['spans']]

for kind in ['regular', 'cropped']:
    for rotation in [0, 90, 180, 270]:
        prefix = f'{kind}-{rotation}'
        metadata = json.loads((root / f'{prefix}.json').read_text())
        original_path = root / f'{prefix}-original.pdf'
        original_hash = hashlib.sha256(original_path.read_bytes()).hexdigest()
        source = fitz.open(original_path)
        target_path = root / f'{prefix}-edited.pdf'
        target = fitz.open(target_path)
        assert target[0].rotation == source[0].rotation == rotation
        assert target[0].cropbox == source[0].cropbox
        assert target[0].mediabox == source[0].mediabox
        expected = ['Moved this paragraph with', 'more words.', 'Second line.']
        for text in [target[0].get_text(), PdfReader(target_path).pages[0].extract_text()]:
            assert 'Original words.' not in text and 'Next line.' not in text, text
            assert text.count('Neighbor stays.') == 1
            for line in expected:
                assert text.count(line) == 1, text
        source_spans, target_spans = spans(source[0]), spans(target[0])
        before = next(item for item in source_spans if item[0]['text'] == 'Neighbor stays.')
        after = next(item for item in target_spans if item[0]['text'] == 'Neighbor stays.')
        assert before == after, (prefix, before, after)
        original_style = next(item for item in source_spans if item[0]['text'] == 'Original words.')
        origins = []
        for index, text in enumerate(expected):
            # Readers may retain the space at a soft wrap; geometry/style remain exact.
            span, direction = next(item for item in target_spans if item[0]['text'].rstrip(' ') == text)
            assert direction == original_style[1] == (1.0, 0.0)
            for key in ['font', 'size', 'color', 'flags']:
                assert span[key] == original_style[0][key]
            assert abs(span['origin'][0] - metadata['raw']['x']) < 0.1
            assert abs(span['origin'][1] - metadata['raw']['y'] - 12 - index * 14.4) < 0.1
            origins.append(span['origin'])
        pixmaps = [page.get_pixmap(matrix=fitz.Matrix(2, 2), colorspace=fitz.csRGB)
                   for page in [source[0], target[0]]]
        images = [Image.frombytes('RGB', (pix.width, pix.height), pix.samples) for pix in pixmaps]
        assert images[0].size == images[1].size
        difference = ImageChops.difference(*images)
        assert difference.getbbox(), 'Expected an actual text change'
        draw = ImageDraw.Draw(difference)
        # Metadata uses already-rotated displayed axes; do not rotate these again.
        for key in ['old', 'new']:
            rect = metadata[key]
            draw.rectangle((int(rect['x'] * 2) - 3, int(rect['y'] * 2) - 3,
                            int((rect['x'] + rect['width']) * 2) + 3,
                            int((rect['y'] + rect['height']) * 2) + 3), fill=(0, 0, 0))
        assert difference.getbbox() is None, (prefix, difference.getbbox())
        assert hashlib.sha256(original_path.read_bytes()).hexdigest() == original_hash
        report.append(dict(fixture=prefix, rotation=rotation, text_readers=['MuPDF', 'pypdf'],
                           origins=origins, font_and_direction_unchanged=True,
                           neighbor_geometry_unchanged=True, crop_unchanged=True,
                           outside_pixels_unchanged=True, source_sha256=original_hash))
(root / 'independent-results.json').write_text(json.dumps(report, indent=2))
print(json.dumps(dict(passed=True, fixtures=len(report), results=report)))
