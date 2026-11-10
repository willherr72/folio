"""Check rotated Latin group exports against independently authored references."""
import argparse
import json
import runpy
from pathlib import Path

from pypdf import PdfWriter
from pypdf.generic import ByteStringObject, ContentStream, NameObject


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    helpers = runpy.run_path(str(Path(__file__).with_name('verify-group-boundaries.py')))
    compare, digest = helpers['compare'], helpers['digest']
    cases = []
    for rotation in [0, 90, 180, 270]:
        for cut in range(1, 8):
            label = f'r{rotation}-s{cut}'
            original = args.input / f'{label}-source.pdf'
            unchanged = args.input / f'{label}-noop.pdf'
            edited = args.input / f'{label}-edited.pdf'
            original_hash = digest(original)
            compare(unchanged, original)
            writer = PdfWriter(clone_from=original)
            page = writer.pages[0]
            assert page.rotation == rotation
            assert list(page.cropbox) == [10, 20, 290, 380]
            content = ContentStream(page['/Contents'].get_object(), writer)
            shows = [i for i, (_, op) in enumerate(content.operations) if op == b'Tj']
            assert len(shows) == 3 and shows[1] == shows[0] + 1
            # The public ReportLab font maps byte 0x01 to U+00E9.
            content.operations[shows[0]] = ([ByteStringObject(b'Edit caf\x01')], b'Tj')
            del content.operations[shows[1]]
            # Native regeneration omits ReportLab's unused final line movement.
            content.operations = [value for i, value in enumerate(content.operations)
                                  if not (value[1] == b'T*' and i + 1 < len(content.operations)
                                          and content.operations[i + 1][1] == b'ET')]
            page[NameObject('/Contents')] = writer._add_object(content)
            reference = args.output / f'{label}-reference.pdf'
            writer.write(reference)
            compare(edited, reference)
            assert digest(original) == original_hash
            cases.append(dict(rotation=rotation, cut=cut, passed=True,
                              originalSha256=original_hash, unchangedSha256=digest(unchanged),
                              editedSha256=digest(edited), referenceSha256=digest(reference)))
    result = dict(passed=True, cases=cases, mupdf=helpers['fitz'].VersionBind,
                  checks='Exact text, glyph geometry, two raster scales and embedded font bytes')
    (args.output / 'results.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(passed=True, cases=len(cases))))


if __name__ == '__main__':
    main()
