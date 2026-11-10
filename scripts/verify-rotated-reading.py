"""Verify unchanged exported PDF content and independent rotated reader text."""
import argparse
import json
from pathlib import Path
import runpy

from pypdf import PdfReader


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    helper = runpy.run_path(str(Path(__file__).with_name('verify-group-boundaries.py')))
    native = json.loads((args.input / 'reading.json').read_text(encoding='utf-8'))
    assert len(native) == 28
    cases = []
    for record in native:
        label = record['label']
        original = args.input / f'{label}-source.pdf'
        saved = args.input / f'{label}-saved.pdf'
        helper['compare'](saved, original)
        with helper['fitz'].open(original) as pdf:
            mupdf_text = pdf[0].get_text()
        pypdf_text = PdfReader(original).pages[0].extract_text()
        # Readers differ in terminal line endings; no spaces or internal breaks
        # are discarded. Exact source/export equality is separately checked above.
        expected = 'Café old\nNeighbor'
        for text in [record['reader'], mupdf_text, pypdf_text]:
            assert text.replace('\r\n', '\n').rstrip('\n') == expected, (label, text)
        assert len(record['search']['matches']) == 1
        cases.append(dict(label=label, passed=True, sourceSha256=helper['digest'](original),
                          savedSha256=helper['digest'](saved), reader=record['reader'],
                          raw=record['raw'], independentText=expected))
    result = dict(passed=True, cases=cases, mupdf=helper['fitz'].VersionBind,
                  checks='Reader text/search; source-export exact text/glyphs/raster at two scales/font bytes')
    (args.output / 'results.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(passed=True, cases=len(cases))))


if __name__ == '__main__':
    main()
