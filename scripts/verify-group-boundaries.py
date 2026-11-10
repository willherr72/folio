"""Verify authored embedded-font group exports without using Folio or PDFium."""
import argparse
import hashlib
import json
from pathlib import Path

import fitz
from pypdf import PdfReader, PdfWriter
from pypdf.generic import ByteStringObject, ContentStream, NameObject


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def programs(reader):
    result = set()
    for page in reader.pages:
        for ref in page['/Resources']['/Font'].values():
            font = ref.get_object()
            if '/DescendantFonts' in font:
                font = font['/DescendantFonts'][0].get_object()
            if '/FontDescriptor' in font:
                descriptor = font['/FontDescriptor'].get_object()
                for key in ['/FontFile', '/FontFile2', '/FontFile3']:
                    if key in descriptor:
                        result.add(hashlib.sha256(descriptor[key].get_data()).hexdigest())
    return result


def glyphs(page):
    return [char for block in page.get_text('rawdict')['blocks'] if 'lines' in block
            for line in block['lines'] for span in line['spans'] for char in span['chars']]


def compare(actual, reference):
    a_reader, b_reader = PdfReader(actual), PdfReader(reference)
    assert len(a_reader.pages) == len(b_reader.pages) == 1
    with fitz.open(actual) as a, fitz.open(reference) as b:
        assert a[0].get_text() == b[0].get_text()
        assert glyphs(a[0]) == glyphs(b[0]), 'Independent glyph geometry differs'
        for scale in [2, 1000 / a[0].rect.width]:
            x, y = [p.get_pixmap(matrix=fitz.Matrix(scale, scale), alpha=False)
                    for p in [a[0], b[0]]]
            assert (x.width, x.height, x.samples) == (y.width, y.height, y.samples)
    assert a_reader.pages[0].extract_text() == b_reader.pages[0].extract_text()
    assert programs(a_reader) == programs(b_reader) and programs(a_reader)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    cases = []
    for cut in range(1, 8):
        original = args.input / f'split-{cut}-original.pdf'
        unchanged = args.input / f'split-{cut}-noop.pdf'
        edited = args.input / f'split-{cut}-edited.pdf'
        original_hash = digest(original)
        compare(unchanged, original)
        writer = PdfWriter(clone_from=original)
        page = writer.pages[0]
        content = ContentStream(page['/Contents'].get_object(), writer)
        shows = [i for i, (_, op) in enumerate(content.operations) if op == b'Tj']
        assert len(shows) == 3 and shows[1] == shows[0] + 1
        # This public ReportLab fixture maps encoded 0x01 to U+00E9.
        content.operations[shows[0]] = ([ByteStringObject(b'Edit caf\x01')], b'Tj')
        del content.operations[shows[1]]
        # ReportLab emits an unused line move before ET. Omit that movement in
        # the independently authored changed reference: pypdf exposes it as a
        # trailing newline, whereas native regeneration writes only painted text.
        # The unchanged case above still compares the exact original extraction.
        content.operations = [value for i, value in enumerate(content.operations)
                              if not (value[1] == b'T*' and i + 1 < len(content.operations)
                                      and content.operations[i + 1][1] == b'ET')]
        page[NameObject('/Contents')] = writer._add_object(content)
        reference = args.output / f'split-{cut}-reference.pdf'
        writer.write(reference)
        compare(edited, reference)
        assert digest(original) == original_hash
        cases.append(dict(cut=cut, passed=True, originalSha256=original_hash,
                          unchangedSha256=digest(unchanged), editedSha256=digest(edited)))
    result = dict(passed=True, cases=cases, mupdf=fitz.VersionBind,
                  checks='Exact text, glyph geometry, two raster scales and embedded font bytes')
    (args.output / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(dict(passed=True, cases=len(cases))))


if __name__ == '__main__':
    main()
