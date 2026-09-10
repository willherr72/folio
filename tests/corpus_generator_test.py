"""Behavior contracts for the development-only compatibility corpus generator."""
import hashlib
import importlib.util
import json
import re
from pathlib import Path
import tempfile
import sys
sys.dont_write_bytecode = True
import unittest
import fitz

SCRIPT = Path(__file__).parents[1] / 'scripts' / 'corpus-generate.py'

class CorpusGeneratorTests(unittest.TestCase):
    def test_committed_fixture_and_font_checksums(self):
        directory = SCRIPT.parents[1] / 'tests/fixtures/corpus'
        manifest = json.loads((directory / 'manifest.json').read_text(encoding='utf-8'))
        for item in manifest['fixtures'] + [manifest['font']]:
            self.assertEqual(hashlib.sha256((directory / item['file']).read_bytes()).hexdigest(), item['sha256'])

    def test_small_suite_repeats_exact_bytes_and_has_semantic_manifest(self):
        spec = importlib.util.spec_from_file_location('corpus_generate', SCRIPT)
        generator = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(generator)
        with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
            first = generator.generate(Path(a), large=False)
            second = generator.generate(Path(b), large=False)
            self.assertEqual(first, second)
            self.assertEqual(len(first['fixtures']), 4)
            for fixture in first['fixtures']:
                path = Path(a) / fixture['file']
                self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), fixture['sha256'])
                self.assertEqual(path.read_bytes(), (Path(b) / fixture['file']).read_bytes())
                with fitz.open(path) as pdf:
                    self.assertEqual(len(pdf), fixture['pages'])
                    if fixture['kind'] == 'scan':
                        self.assertTrue(all(not page.get_text().strip() and page.get_images() for page in pdf))
                    else:
                        for sample in fixture['samples']:
                            self.assertIn(sample['text'], pdf[sample['page']].get_text())
                    if fixture['kind'] == 'embedded-font':
                        self.assertTrue(any(pdf.extract_font(font[0])[3] for font in pdf[0].get_fonts()))
                        for xref in range(1, pdf.xref_length()):
                            if pdf.xref_is_stream(xref):
                                stream = pdf.xref_stream(xref)
                                if b'begincmap' in stream:
                                    self.assertTrue(all(len(token) % 4 == 0 for token in re.findall(rb'<([0-9a-fA-F]+)>', stream)), 'ToUnicode must encode complete UTF-16BE units')
                    if fixture['kind'] == 'mixed-geometry':
                        self.assertEqual({page.rotation for page in pdf}, {0, 90, 180, 270})
                        self.assertTrue(any(page.cropbox != page.mediabox for page in pdf))

if __name__ == '__main__':
    unittest.main()
