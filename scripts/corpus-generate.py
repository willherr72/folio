"""Generate synthetic, redistributable Folio PDFs. Development dependencies only.

Run: python scripts/corpus-generate.py [--large] [--output DIRECTORY]
Large generation writes to ignored artifacts/corpus by default; never downloads content.
"""
import argparse
import hashlib
import io
import json
import platform
import random
import re
import shutil
from pathlib import Path

import fitz
import PIL
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
FONT = ROOT / 'tests/fixtures/corpus/fonts/DejaVuSerif.ttf'
SEED = 6062026


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(pdf, path):
    # PyMuPDF 1.27.1 emits scalar hex for supplementary glyph mappings. PDF
    # ToUnicode values require UTF-16BE surrogate pairs, including unused glyphs.
    for xref in range(1, pdf.xref_length()):
        if pdf.xref_is_stream(xref) and pdf.xref_get_key(xref, "Subtype")[1] != "/Image":
            stream = pdf.xref_stream(xref)
            if b'begincmap' in stream:
                fixed = re.sub(rb'<([0-9a-fA-F]{5,6})>',
                               lambda match: b'<' + chr(int(match[1], 16)).encode('utf-16-be').hex().encode('ascii') + b'>', stream)
                if fixed != stream:
                    pdf.update_stream(xref, fixed)
    pdf.set_metadata({'title': path.stem, 'author': 'Folio synthetic corpus',
                      'creator': 'corpus-generate.py v1', 'producer': 'Folio corpus',
                      'creationDate': 'D:20260101000000Z', 'modDate': 'D:20260101000000Z'})
    pdf.save(path, garbage=4, deflate=True, no_new_id=True)
    pdf.close()


def dense(path, count):
    pdf = fitz.open()
    for index in range(count):
        page = pdf.new_page(width=612, height=792)
        page.insert_text((32, 28), f'FOLIO DENSE PAGE {index + 1:04d}', fontsize=12)
        shape = page.new_shape()
        for row in range(61):
            shape.draw_line((30, 45 + row * 11), (582, 45 + row * 11))
        for col in range(9):
            shape.draw_line((30 + col * 69, 45), (30 + col * 69, 705))
        shape.finish(width=.25, color=(.45, .45, .45))
        shape.commit()
        for row in range(60):
            line = '   '.join(f'{index + 1:03d}-{row + 1:02d}-{col + 1:02d}' for col in range(8))
            page.insert_text((33, 53 + row * 11), line, fontname='cour', fontsize=6.5)
        page.insert_text((32, 750), 'Synthetic table. No personal or confidential information.', fontsize=9)
    save(pdf, path)


def mixed(path):
    pdf = fitz.open()
    for index, (width, height, rotation) in enumerate([(612, 792, 0), (842, 595, 90), (420, 595, 180), (900, 900, 270)]):
        page = pdf.new_page(width=width, height=height)
        page.insert_text((80, 110), f'FOLIO GEOMETRY PAGE {index + 1:04d}', fontsize=13)
        page.insert_text((80, 140), 'Crop, rotation, page size and editable annotation probe.', fontsize=9)
        page.draw_rect(fitz.Rect(70, 90, 320, 165), color=(0, .2, .8), width=1)
        mark = page.add_highlight_annot(fitz.Rect(80, 100, 260, 115))
        mark.set_info(content=f'Corpus highlight {index + 1}')
        mark.set_colors(stroke=(1, .8, 0))
        mark.set_opacity(.3)
        mark.set_flags(4)
        mark.update()
        note = page.add_text_annot((90, 175), f'Corpus comment {index + 1}')
        note.set_flags(4)
        note.update()
        page.set_cropbox(fitz.Rect(30, 40, width - 20, height - 30))
        page.set_rotation(rotation)
    save(pdf, path)


def embedded(path):
    pdf = fitz.open()
    lines = ['FOLIO EMBEDDED FONT', 'DejaVu Serif embedded TrueType Unicode',
             'Latin: café naïve Straße Ångström', 'Greek: Ελληνικά αβγδε Ω',
             'Cyrillic: Пример текста', 'Math: ∑ ∫ √ ∞ ≠ ≤ ≥']
    for index in range(2):
        page = pdf.new_page(width=612, height=792)
        page.insert_font(fontname='CorpusSerif', fontfile=str(FONT))
        for row, line in enumerate(lines):
            page.insert_text((40, 60 + row * 48), line, fontname='CorpusSerif', fontsize=18 + index * 3)
    save(pdf, path)


def scanned(path, count, dpi):
    pdf = fitz.open()
    width, height = round(8.5 * dpi), round(11 * dpi)
    font = ImageFont.truetype(str(FONT), round(dpi * .17))
    # 32 paper-noise tones, independently seeded pages, and unique labels prevent
    # image deduplication. Pixels and page content are wholly synthetic.
    palette = bytes(224 + value % 32 for value in range(256))
    for index in range(count):
        noise = random.Random(SEED + index).randbytes(width * height).translate(palette)
        bitmap = Image.frombytes('L', (width, height), noise)
        draw = ImageDraw.Draw(bitmap)
        draw.text((dpi // 2, dpi // 2), f'FOLIO SYNTHETIC SCAN {index + 1:04d}', font=font, fill=15)
        for row in range(36):
            y = dpi + row * dpi // 4
            draw.text((dpi // 2, y), f'ROW {row + 1:02d}  Review amount {(index + 1) * (row + 7):06d}  Synthetic record', font=font, fill=30)
            draw.line((dpi // 2, y + dpi // 5, width - dpi // 2, y + dpi // 5), fill=120, width=1)
        stream = io.BytesIO()
        bitmap.save(stream, format='PNG', compress_level=6)
        page = pdf.new_page(width=612, height=792)
        page.insert_image(page.rect, stream=stream.getvalue())
    save(pdf, path)


def generate(output, large=False):
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    (output / 'fonts').mkdir(exist_ok=True)
    for source, destination in [(FONT, output / 'fonts' / FONT.name),
                                (FONT.parent / 'LICENSE_DEJAVU.txt', output / 'fonts/LICENSE_DEJAVU.txt'),
                                (ROOT / 'LICENSE', output / 'LICENSE_CONTENT.txt')]:
        if source.resolve() != destination.resolve():
            shutil.copyfile(source, destination)
    definitions = [('dense-tables.pdf', 'dense-text', dense, (3,), {'rows': 60, 'columns': 8}),
                   ('mixed-geometry.pdf', 'mixed-geometry', mixed, (), {'rotations': [0, 90, 180, 270], 'cropInset': [30, 40, 20, 30]}),
                   ('embedded-font.pdf', 'embedded-font', embedded, (), {'font': 'DejaVuSerif.ttf', 'scripts': ['Latin', 'Greek', 'Cyrillic', 'Mathematical symbols']}),
                   ('scan-small.pdf', 'scan', scanned, (1, 100), {'dpi': 100, 'seed': SEED, 'paperNoiseTones': 32})]
    if large:
        definitions += [('dense-300-pages.pdf', 'dense-text', dense, (300,), {'rows': 60, 'columns': 8}),
                        ('scan-40-pages.pdf', 'scan', scanned, (40, 200), {'dpi': 200, 'seed': SEED, 'paperNoiseTones': 32})]
    fixtures = []
    for name, kind, make, arguments, settings in definitions:
        path = output / name
        make(path, *arguments)
        with fitz.open(path) as pdf:
            samples = []
            for index in sorted({0, len(pdf) // 2, len(pdf) - 1}):
                text = pdf[index].get_text().splitlines()
                samples.append({'page': index, 'text': text[0] if text else ''})
            fixtures.append({'file': name, 'kind': kind, 'pages': len(pdf), 'bytes': path.stat().st_size,
                             'sha256': sha256(path), 'settings': settings, 'samples': samples,
                             'textProbes': ['café naïve Straße Ångström', 'Ελληνικά αβγδε Ω', 'Пример текста', '∑ ∫ √ ∞ ≠ ≤ ≥'] if kind == 'embedded-font' else [],
                             'dimensions': [[page.rect.width, page.rect.height] for page in pdf],
                             'license': 'Repository license for synthetic content; bundled DejaVu license for font/image glyphs',
                             'source': 'Generated locally by scripts/corpus-generate.py; no external PDF/image/text source'})
    manifest = {'schemaVersion': 1, 'generatorVersion': 1, 'seed': SEED,
                'toolchain': {'python': platform.python_version(), 'pymupdf': fitz.VersionBind, 'pillow': PIL.__version__},
                'font': {'file': 'fonts/DejaVuSerif.ttf', 'sha256': sha256(FONT),
                         'licenseFile': 'fonts/LICENSE_DEJAVU.txt', 'source': 'https://dejavu-fonts.github.io/',
                         'distribution': 'Unmodified DejaVu Serif distributed with Matplotlib; exact bytes bundled for offline regeneration'},
                'fixtures': fixtures}
    (output / 'manifest.json').write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    return manifest


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--large', action='store_true')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    destination = args.output or ROOT / ('artifacts/corpus' if args.large else 'tests/fixtures/corpus')
    result = generate(destination, large=args.large)
    print(json.dumps({'output': str(destination), 'fixtures': [{'file': f['file'], 'pages': f['pages'], 'bytes': f['bytes']} for f in result['fixtures']]}, indent=2))
