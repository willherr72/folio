"""Independent pypdf/MuPDF oracle for the synthetic native replacement corpus."""
import argparse
import hashlib
import json
from pathlib import Path

import fitz
from pypdf import PdfReader
import importlib.util

spec = importlib.util.spec_from_file_location('replacement_probe', Path(__file__).with_name('probe-page-replacement.py'))
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    records = []
    for result in sorted(args.corpus.glob('*-result.pdf')):
        label = result.name.removesuffix('-result.pdf')
        target, source = [args.corpus / f'{label}-{kind}.pdf' for kind in ['target', 'incoming']]
        hashes = {p.name: digest(p) for p in [target, source, result]}
        reference = args.output / f'{label}-reference.pdf'
        # Independent direct composition renames incoming font operators and keeps
        # target resources, instead of replacing resources and isolating old APs.
        matrix = probe.compose(target, source, reference, True)
        assert probe.metadata(result) == probe.metadata(target), label
        before, after = PdfReader(target), PdfReader(result)
        for key in ['/MediaBox', '/CropBox', '/Rotate', '/UserUnit']:
            assert before.pages[0].get(key) == after.pages[0].get(key), (label, key)
        with fitz.open(result) as actual, fitz.open(reference) as expected, fitz.open(target) as original:
            assert actual[0].get_text() == expected[0].get_text(), label
            assert 'New content' in actual[0].get_text(), label
            for scale in [1, 2]:
                a = actual[0].get_pixmap(matrix=fitz.Matrix(scale, scale))
                b = expected[0].get_pixmap(matrix=fitz.Matrix(scale, scale))
                assert (a.width, a.height, a.samples) == (b.width, b.height, b.samples), (label, scale)
                assert actual[1].get_pixmap(matrix=fitz.Matrix(scale, scale)).samples == original[1].get_pixmap(matrix=fitz.Matrix(scale, scale)).samples, label
        assert hashes == {p.name: digest(p) for p in [target, source, result]}
        records.append(dict(label=label, passed=True, matrix=matrix, sha256=hashes, referenceSha256=digest(reference)))
    assert len(records) == 32, f'Expected 32 native cases, found {len(records)}'
    report = dict(passed=True, cases=records, mupdf=fitz.VersionBind)
    (args.output/'result.json').write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(dict(passed=True, cases=len(records))))


if __name__ == '__main__':
    main()
