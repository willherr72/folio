"""Independent image-overlay crop/rotation, attachment and page-fidelity checks."""
from pathlib import Path
import hashlib
import io
import json

import fitz
import numpy as np
import pypdf
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'artifacts/image-overlay-native'


def rotate(x, y, width, height, rotation):
    return {0: (x, y), 90: (height-y, x), 180: (width-x, height-y),
            270: (y, width-x)}[rotation]


def raster(page):
    pix = page.get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False, annots=True)
    return np.frombuffer(pix.samples, np.uint8).reshape(pix.height, pix.width, 3).copy()


def main():
    results = []
    for intrinsic in [0, 90, 180, 270]:
        width, height = (300, 200) if intrinsic in [90, 270] else (200, 300)
        for workspace in [0, 90, 180, 270]:
            with fitz.open(OUTPUT / f'source-{intrinsic}.pdf') as original:
                original[0].set_rotation((intrinsic + workspace) % 360)
                baseline = raster(original[0])
            for rotation in [0, 90, 180, 270]:
                path = OUTPUT / f'image-{intrinsic}-{workspace}-{rotation}.pdf'
                assert path.is_file(), path
                with fitz.open(path) as doc:
                    assert 'Original native text' in doc[0].get_text(), path.name
                    actual = raster(doc[0])
                corners = []
                for x, y in [(0, 0), (40, 0), (40, 20), (0, 20)]:
                    dx, dy = {0: (x, y), 90: (-y, x), 180: (-x, -y),
                              270: (y, -x)}[rotation]
                    corners.append(rotate(width/2+dx, height/2+dy, width, height, workspace))
                left, top = np.min(corners, axis=0)
                right, bottom = np.max(corners, axis=0)
                outside = np.ones(actual.shape[:2], dtype=bool)
                outside[int(top*2)-2:int(bottom*2)+2, int(left*2)-2:int(right*2)+2] = False
                assert actual.shape == baseline.shape, path.name
                assert np.array_equal(actual[outside], baseline[outside]), f'{path.name}: surrounding pixels changed'
                center = actual[int(top+bottom), int(left+right)].astype(int)
                assert np.max(np.abs(center-[102, 0, 127])) <= 2, (path.name, center)
                pdf = pypdf.PdfReader(path)
                assert 'Original native text' in pdf.pages[0].extract_text()
                annot = pdf.pages[0]['/Annots'][0].get_object()
                metadata = json.loads(annot['/Folio'])
                assert metadata['version'] == 5 and metadata['overlay']['type'] == 'image'
                attachment = annot['/FolioImageAsset'].get_object()
                original_bytes = attachment.get_data()
                assert attachment['/Params']['/Size'] == len(original_bytes)
                assert hashlib.sha256(original_bytes).hexdigest() == metadata['overlay']['assetId']
                assert str(attachment['/FolioImageId']) == metadata['overlay']['assetId']
                assert attachment['/FolioVersion'] == 1 and attachment['/Subtype'] == '/image/png'
                source = Image.open(io.BytesIO(original_bytes)).convert('RGBA')
                assert source.size == (4, 2) and source.getpixel((0, 0)) == (204, 0, 0, 128)
                form = annot['/AP']['/N']['/Resources']['/XObject']['/FolioImage']
                pixels = form['/Resources']['/XObject']['/Im0']
                assert pixels['/BitsPerComponent'] == 8 and pixels['/ColorSpace'] == '/DeviceRGB'
                assert pixels.get_data() == bytes([204, 0, 0])*8
                assert pixels['/SMask'].get_data() == bytes([128])*8
                results.append({'file': path.name, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                                'outsidePixelsUnchanged': True, 'nativeText': True,
                                'originalAssetIdentity': True, 'alphaAndSamples': True})
    assert len(results) == 64
    for flatten in [False, True]:
        for image_last in [False, True]:
            path = OUTPUT / f'order-{str(flatten).lower()}-{str(image_last).lower()}.pdf'
            assert path.is_file(), path
            with fitz.open(path) as doc:
                actual = raster(doc[0])
                expected = [103, 0, 127] if image_last else [0, 0, 255]
                assert np.max(np.abs(actual[120, 120].astype(int)-expected)) <= 2, path.name
            pdf = pypdf.PdfReader(path)
            if flatten:
                assert not pdf.pages[0].get('/Annots')
                assert 'Overlay text' in pdf.pages[0].extract_text()
            else:
                kinds = [a.get_object()['/Subtype'] for a in pdf.pages[0]['/Annots']]
                assert kinds == (['/FreeText', '/Ink', '/Stamp'] if image_last else ['/FreeText', '/Stamp', '/Ink'])
            results.append({'file': path.name, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                            'textInkImageOrder': True, 'flattened': flatten})
    fixture_root = ROOT / 'tests/fixtures/image-assets'
    corpus = json.loads((fixture_root / 'manifest.json').read_text())['cases']
    for name in [case['name'] for case in corpus] + ['rgba16']:
        path = OUTPUT / f'roundtrip-{name}.pdf'
        assert path.is_file(), path
        with fitz.open(path) as doc:
            actual = raster(doc[0])
        outside = np.ones(actual.shape[:2], dtype=bool)
        outside[78:162, 38:202] = False
        outside[198:282, 38:202] = False
        assert np.all(actual[outside] == 255), path.name
        assert np.array_equal(actual[120, 120], actual[240, 120]), path.name
        pdf = pypdf.PdfReader(path)
        annotations = [a.get_object() for a in pdf.pages[0]['/Annots']]
        assert len(annotations) == 2
        assert annotations[0].raw_get('/FolioImageAsset') == annotations[1].raw_get('/FolioImageAsset')
        attachment = annotations[0]['/FolioImageAsset']
        original_bytes = attachment.get_data()
        metadata = json.loads(annotations[0]['/Folio'])
        assert hashlib.sha256(original_bytes).hexdigest() == metadata['overlay']['assetId']
        forms = [a['/AP']['/N']['/Resources']['/XObject'] for a in annotations]
        assert forms[0].raw_get('/FolioImage') == forms[1].raw_get('/FolioImage')
        pixels = forms[0]['/FolioImage']['/Resources']['/XObject']['/Im0']
        if name == 'rgba16':
            assert pixels['/BitsPerComponent'] == 16 and pixels['/SMask']['/BitsPerComponent'] == 16
            assert pixels.get_data() == bytes.fromhex('ffff20010000')*8
            assert pixels['/SMask'].get_data() == bytes.fromhex('8001')*8
            assert np.max(np.abs(actual[120, 120].astype(int)-[255, 143, 127])) <= 2
        else:
            assert original_bytes == (fixture_root / f'{name}.jpg').read_bytes()
            assert pixels['/Filter'] == '/DCTDecode' and pixels._data == original_bytes
        results.append({'file': path.name, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                        'originalAssetIdentity': True, 'sharedResources': True, 'outsidePixelsUnchanged': True})
    assert len(results) == 84
    highlight_pixels = []
    for name in ['highlight-editable', 'highlight-flat']:
        path = OUTPUT / f'{name}.pdf'
        with fitz.open(path) as doc:
            highlight_pixels.append(raster(doc[0]))
        results.append({'file': path.name, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                        'annotationAboveImageContent': True})
    assert np.array_equal(highlight_pixels[0], highlight_pixels[1]), 'editable/flattened highlight compositing differs'
    assert np.max(np.abs(highlight_pixels[0][120, 120].astype(int)-[138, 127, 127])) <= 3
    report = {'mupdf': fitz.VersionBind, 'pypdf': pypdf.__version__, 'verifiedPdfCount': len(results), 'pdfs': results}
    (OUTPUT / 'independent-results.json').write_text(json.dumps(report, indent=2)+'\n')
    print('Verified 86 image-overlay PDFs: crop/rotation, native text/outside pixels, attachments/alpha, drawing order, original JPEGs/16-bit PNG and highlight compositing.')


if __name__ == '__main__':
    main()
