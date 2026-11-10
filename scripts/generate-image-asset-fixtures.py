"""Generate original JPEG decoder fixtures and independent Pillow reference pixels."""
from pathlib import Path
import hashlib
import json
import PIL
from PIL import Image

root = Path(__file__).resolve().parent.parent / 'tests/fixtures/image-assets'
root.mkdir(parents=True, exist_ok=True)
records = []
cases = [('tiny', 1, 1, False, 0), ('odd', 17, 9, False, 0),
         ('progressive', 17, 9, True, 0), ('restart', 80, 40, False, 1),
         ('progressive-restart', 80, 40, True, 1)]
for mode in ['RGB', 'L', 'CMYK']:
    for label, width, height, progressive, restart in cases:
        image = Image.new('RGB', (width, height))
        image.putdata([((x * 7 + y * 11) % 256, (x * 3 + y * 17) % 256,
                        (x * 19 + y * 5) % 256)
                       for y in range(height) for x in range(width)])
        name = f'{mode.lower()}-{label}'
        jpeg = root / f'{name}.jpg'
        image.convert(mode).save(jpeg, quality=92, progressive=progressive,
                                 restart_marker_blocks=restart, subsampling=0)
        with Image.open(jpeg) as decoded:
            decoded.convert('RGB').save(root / f'{name}.png')
        if restart:
            data = jpeg.read_bytes()
            assert any(bytes([255, marker]) in data for marker in range(0xd0, 0xd8))
        records.append({'name': name, 'width': width, 'height': height,
                        'jpegSha256': hashlib.sha256(jpeg.read_bytes()).hexdigest()})
(root / 'manifest.json').write_text(json.dumps({
    'generator': 'scripts/generate-image-asset-fixtures.py',
    'pillowVersion': PIL.__version__,
    'description': 'Original deterministic pixels; JPEG encoding and reference decoding by Pillow/libjpeg.',
    'cases': records,
}, indent=2) + '\n', encoding='utf-8')
print(f'Generated {len(records)} JPEG/reference pairs in {root}')
