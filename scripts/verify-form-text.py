"""Check nested-form exports against independently authored PDF references.

Generate fixtures with the native form_text tests. Comparisons are exact;
reader whitespace and raster differences are failures, not normalized away.
"""
import argparse
import hashlib
import json
from pathlib import Path

import fitz
import pypdf

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--input', type=Path, default=Path('artifacts/form-text-native'))
parser.add_argument('--output', type=Path, default=Path('artifacts/form-text-native/reader-results.json'))
parser.add_argument('--expected-cases', type=int, required=True)
args = parser.parse_args()
exports = sorted(args.input.glob('*-exported.pdf'))
references = sorted(args.input.glob('*-reference.pdf'))
assert len(exports) == args.expected_cases, f'Expected {args.expected_cases} exports; found {len(exports)}'
assert len(references) == args.expected_cases, f'Expected {args.expected_cases} references; found {len(references)}'
assert args.expected_cases > 0, 'At least one independently authored reference is required'
results = []
for output in exports:
    reference = output.with_name(output.name.replace('-exported.pdf', '-reference.pdf'))
    assert reference.is_file(), f'Missing reference: {reference}'
    row = {'case': output.stem.removesuffix('-exported'),
           'exportSha256': hashlib.sha256(output.read_bytes()).hexdigest(),
           'referenceSha256': hashlib.sha256(reference.read_bytes()).hexdigest(),
           'pages': []}
    with fitz.open(output) as actual, fitz.open(reference) as expected:
        assert len(actual) == len(expected), f'{output}: page count differs'
        actual_pypdf = pypdf.PdfReader(output)
        expected_pypdf = pypdf.PdfReader(reference)
        for index, (a_page, b_page) in enumerate(zip(actual, expected)):
            a = a_page.get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False)
            b = b_page.get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False)
            a_text = a_page.get_text(sort=False)
            b_text = b_page.get_text(sort=False)
            a_pypdf = actual_pypdf.pages[index].extract_text()
            b_pypdf = expected_pypdf.pages[index].extract_text()
            row['pages'].append({'page': index,
                'mupdfExactText': a_text == b_text,
                'mupdfExactGlyphGeometry': a_page.get_text('rawdict') == b_page.get_text('rawdict'),
                'mupdfExactRaster': (a.width, a.height, a.samples) == (b.width, b.height, b.samples),
                'pypdfExactText': a_pypdf == b_pypdf,
                'mupdfText': a_text, 'referenceMupdfText': b_text,
                'pypdfText': a_pypdf, 'referencePypdfText': b_pypdf,
                'mupdfRasterSha256': hashlib.sha256(a.samples).hexdigest(),
                'referenceMupdfRasterSha256': hashlib.sha256(b.samples).hexdigest()})
    row['passed'] = all(p[k] for p in row['pages'] for k in ('mupdfExactText', 'mupdfExactGlyphGeometry', 'mupdfExactRaster', 'pypdfExactText'))
    results.append(row)
report = {'mupdf': fitz.VersionBind, 'pypdf': pypdf.__version__, 'cases': len(results),
          'passed': all(r['passed'] for r in results), 'results': results}
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(json.dumps({k: v for k, v in report.items() if k != 'results'}))
if not report['passed']:
    print(json.dumps([r for r in results if not r['passed']], ensure_ascii=False, indent=2))
    raise SystemExit(1)
