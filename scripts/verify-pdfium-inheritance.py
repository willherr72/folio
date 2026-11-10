"""Independently verify authored references and edited inherited-state outputs."""
import argparse
import hashlib
import json
from pathlib import Path
import runpy

import fitz
from pypdf import PdfReader


def verify(root):
    helpers = runpy.run_path(str(Path(__file__).with_name('verify-form-text-flow.py')))
    equal_page, records = helpers['equal_page'], helpers['records']
    outputs = sorted(root.glob('*-saved.pdf'))
    assert len(outputs) == 15, f'Expected 12 scalar/font and 3 transformed cases, found {len(outputs)}'
    checks = []
    hashes = {}
    for output in outputs:
        prefix = output.name.removesuffix('-saved.pdf')
        source = root / f'{prefix}-source.pdf'
        reference = root / f'{prefix}-reference.pdf'
        for path in [source, reference, output]:
            hashes[path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
        with fitz.open(source) as original, fitz.open(reference) as expected, fitz.open(output) as edited:
            assert len(original) == len(expected) == len(edited) == 2
            equal_page(expected[0], edited[0])
            equal_page(expected[1], edited[1])
            equal_page(original[1], edited[1])
            assert 'Cafe older' in edited[0].get_text(), prefix
        expected, edited, original = PdfReader(reference), PdfReader(output), PdfReader(source)
        assert records(expected) == records(edited), (prefix, 'edited records')
        assert records(original, 1) == records(edited, 1), (prefix, 'untouched page')
        assert [page.extract_text() for page in expected.pages] == [page.extract_text() for page in edited.pages], prefix
        checks.append(prefix)
    result = {'passed': True, 'cases': checks, 'readers': ['MuPDF', 'pypdf'], 'sha256': hashes}
    report = root / 'independent-results.json'
    if report.exists():
        raise SystemExit('Independent report already exists; preserve prior evidence')
    report.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'passed': True, 'cases': len(checks)}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    verify(parser.parse_args().root)
