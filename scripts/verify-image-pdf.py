"""Independent MuPDF/pypdf/LittleCMS checks for image appearance fixtures."""
from pathlib import Path
import hashlib
import io
import json
import os
import fitz
import numpy as np
import pypdf
from PIL import Image, ImageCms

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = Path(os.environ.get('FOLIO_IMAGE_PDF_OUTPUT', ROOT / 'artifacts/image-pdf-native'))


def raster(path):
    with fitz.open(path) as doc:
        assert len(doc) == 1
        pix = doc[0].get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False)
        return np.frombuffer(pix.samples, np.uint8).reshape(pix.height, pix.width, 3).copy()


def near(actual, expected, label, tolerance=4):
    difference = np.max(np.abs(np.array(actual, dtype=int) - np.array(expected, dtype=int)))
    assert difference <= tolerance, (label, actual, expected, int(difference))
    return int(difference)


def main():
    baseline = raster(OUTPUT / 'background.pdf')
    mask = np.ones(baseline.shape[:2], dtype=bool)
    mask[340:400, 100:280] = False
    expected_names = ['alpha8', 'alpha16', 'gamma1', 'custom-white', 'gray-gamma',
                      'rgb-profile', 'gray-profile', 'cmyk-profile']
    expected_names += [f'orientation{i}' for i in range(1, 9)]
    corpus = json.loads((ROOT / 'tests/fixtures/image-assets/manifest.json').read_text())['cases']
    expected_names += [f'corpus-{case["name"]}' for case in corpus]
    results = []
    for name in expected_names:
        path = OUTPUT / f'{name}.pdf'
        actual = raster(path)
        assert np.array_equal(actual[mask], baseline[mask]), f'{name}: pixels changed outside image'
        reader = pypdf.PdfReader(path)
        assert reader.pages[0].extract_text().strip() == 'Untouched original text'
        form = reader.pages[0]['/Resources']['/XObject']['/Placed'].get_object()
        assert list(form['/BBox']) == [0, 0, 1, 1]
        obj = form['/Resources']['/XObject']['/Im0'].get_object()
        assert obj['/Subtype'] == '/Image'
        if name.startswith('alpha'):
            bits = int(name[5:])
            assert obj['/BitsPerComponent'] == bits
            soft = obj['/SMask'].get_object()
            assert soft['/ColorSpace'] == '/DeviceGray' and soft['/BitsPerComponent'] == bits
            assert soft.get_data() == (bytes([0, 128, 255]) if bits == 8 else bytes([0, 0, 128, 1, 255, 255]))
            for x, color in [(130, [0, 0, 255]), (190, [128, 0, 127]), (250, [255, 0, 0])]:
                near(actual[370, x], color, name)
        if name in ['gamma1', 'gray-gamma', 'custom-white']:
            near(actual[370, 190], [188, 188, 188], name)
            assert obj['/ColorSpace'][0] == '/ICCBased'
            assert obj.get_data() == bytes([128] * (1 if name == 'gray-gamma' else (9 if name == 'gamma1' else 3)))
            profile = obj['/ColorSpace'][1].get_object().get_data()
            assert profile[24:36] == bytes([7, 208, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0])
            ImageCms.ImageCmsProfile(io.BytesIO(profile))
        if name.startswith('orientation'):
            colors = [[255, 0, 0], [0, 255, 0], [255, 255, 0], [0, 0, 255],
                      [255, 0, 0], [0, 0, 255], [255, 255, 0], [0, 255, 0]]
            near(actual[352, 130], colors[int(name[11:]) - 1], name)
            assert obj['/Filter'] == '/DCTDecode'
        if name.startswith('corpus-'):
            source = ROOT / 'tests/fixtures/image-assets' / f'{name[7:]}.jpg'
            assert obj['/Filter'] == '/DCTDecode' and obj._data == source.read_bytes()
        results.append({'file': path.name, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                        'nativeText': True, 'outsidePixelsUnchanged': True})

    color_results = []
    for name, extension in [('rgb-profile', 'png'), ('gray-profile', 'png'), ('cmyk-profile', 'jpg')]:
        source = Image.open(OUTPUT / f'{name}.{extension}')
        profile_bytes = source.info['icc_profile']
        profile = ImageCms.ImageCmsProfile(io.BytesIO(profile_bytes))
        expected = ImageCms.profileToProfile(source, profile, ImageCms.createProfile('sRGB'), outputMode='RGB')
        native = Image.open(OUTPUT / f'{name}-pdfium.png')
        actual = raster(OUTPUT / f'{name}.pdf')
        pdf = pypdf.PdfReader(OUTPUT / f'{name}.pdf')
        obj = pdf.pages[0]['/Resources']['/XObject']['/Placed']['/Resources']['/XObject']['/Im0']
        embedded = obj['/ColorSpace'][1].get_object()
        assert embedded.get_data() == profile_bytes
        assert embedded['/N'] == {'rgb-profile': 3, 'gray-profile': 1, 'cmyk-profile': 4}[name]
        max_error = 0
        for u, v in [(0.15, 0.15), (0.5, 0.5), (0.85, 0.85)]:
            reference = expected.getpixel((int(u * source.width), int(v * source.height)))
            max_error = max(max_error, near(actual[int(340 + 60*v), int(100 + 180*u)], reference, name))
            max_error = max(max_error, near(native.getpixel((int(50 + 90*u), int(170 + 30*v))), reference, name))
        color_results.append({'name': name, 'profileBytesRetained': True, 'maxChannelError': max_error})
    report = {'mupdf': fitz.VersionBind, 'pypdf': pypdf.__version__,
              'verifiedPdfCount': len(results), 'pdfs': results, 'independentColorChecks': color_results}
    (OUTPUT / 'independent-results.json').write_text(json.dumps(report, indent=2) + '\n')
    print(f'Verified {len(results)} image PDFs: native text, outside pixels, alpha/depth, orientation, DCT identity and ICC colors.')


if __name__ == '__main__':
    main()
