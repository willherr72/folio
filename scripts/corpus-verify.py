"""Independent pypdf verification of corpus SHA-256, geometry and extraction."""
import argparse
import hashlib
import json
from pathlib import Path
import pypdf
from pypdf import PdfReader


def verify(directory):
    directory = Path(directory)
    manifest = json.loads((directory / 'manifest.json').read_text(encoding='utf-8'))
    font = directory / manifest['font']['file']
    if hashlib.sha256(font.read_bytes()).hexdigest() != manifest['font']['sha256']:
        raise ValueError('Font checksum mismatch')
    results = []
    for fixture in manifest['fixtures']:
        path = directory / fixture['file']
        with path.open('rb') as source:
            digest = hashlib.file_digest(source, 'sha256').hexdigest()
        if digest != fixture['sha256']:
            raise ValueError('PDF checksum mismatch: ' + fixture['file'])
        pdf = PdfReader(path)
        assert len(pdf.pages) == fixture['pages']
        for page, expected in zip(pdf.pages, fixture['dimensions']):
            width, height = float(page.cropbox.width), float(page.cropbox.height)
            if page.rotation % 180:
                width, height = height, width
            assert abs(width - expected[0]) < .01 and abs(height - expected[1]) < .01
        for sample in fixture['samples']:
            assert sample['text'] in pdf.pages[sample['page']].extract_text(), (fixture['file'], sample)
        if fixture['kind'] == 'scan':
            for page in pdf.pages:
                assert not page.extract_text().strip()
                # Resource-level check avoids decoding all 96 MiB of image data.
                images = [obj.get_object() for obj in page['/Resources']['/XObject'].values()]
                assert any(obj.get('/Subtype') == '/Image' for obj in images)
        if fixture['kind'] == 'embedded-font':
            extracted = pdf.pages[0].extract_text()
            for probe in fixture['textProbes']:
                assert probe in extracted, probe
            embedded = False
            for ref in pdf.pages[0]['/Resources']['/Font'].values():
                font_dict = ref.get_object()
                for descendant in font_dict.get('/DescendantFonts', [font_dict]):
                    descriptor = descendant.get_object().get('/FontDescriptor')
                    if descriptor and any(key in descriptor.get_object() for key in ['/FontFile','/FontFile2','/FontFile3']):
                        embedded = True
            assert embedded
        if fixture['kind'] == 'mixed-geometry':
            for page in pdf.pages:
                annotations = [ref.get_object() for ref in page['/Annots']]
                assert any(a['/Subtype'] == '/Highlight' and len(a['/QuadPoints']) == 8 for a in annotations)
                assert any(a['/Subtype'] == '/Text' and int(a['/F']) == 4 for a in annotations)
        results.append({'file': fixture['file'], 'pages': len(pdf.pages), 'sha256': digest, 'passed': True})
    return {'reader': 'pypdf', 'version': pypdf.__version__, 'passed': True, 'fixtures': results}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--report', type=Path)
    args = parser.parse_args()
    result = verify(args.directory)
    encoded = json.dumps(result, indent=2) + '\n'
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(encoded, encoding='utf-8')
    print(encoded)
