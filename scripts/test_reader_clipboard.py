"""Exact comparison tests; never read the live clipboard during tests."""
import importlib.util
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location('clipboard_check', Path(__file__).with_name('check-reader-clipboard.py'))


class ClipboardTests(unittest.TestCase):
    def setUp(self):
        self.assertTrue(Path(spec.origin).exists(), 'Clipboard checker must exist')
        self.check = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.check)

    def test_exact_match(self):
        result = self.check.compare('A  B\r\nC', 'A  B\r\nC')
        self.assertTrue(result['exact'])
        self.assertIsNone(result['firstDifference'])

    def test_whitespace_and_unicode_changes_are_not_normalized(self):
        for expected, actual in [('A  B', 'A B'), ('A B', 'A B\n'),
                                 ('A\nB', 'A\r\nB'), ('é', 'e\u0301'), ('A B', 'A\u00a0B')]:
            with self.subTest(expected=expected, actual=actual):
                result = self.check.compare(expected, actual)
                self.assertFalse(result['exact'])
                self.assertEqual(result['actual'], actual)
                self.assertIsNotNone(result['firstDifference'])

    def test_missing_or_extra_character_is_visible(self):
        self.assertEqual(self.check.compare('AB', 'A')['firstDifference'],
                         {'scalarIndex': 1, 'expected': 'U+0042', 'actual': 'END'})
        self.assertEqual(self.check.compare('A', 'AB')['firstDifference'],
                         {'scalarIndex': 1, 'expected': 'END', 'actual': 'U+0042'})

    def test_fixture_rejects_changed_bytes_duplicate_entries_and_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pdf = root / 'control.pdf'
            pdf.write_bytes(b'fixture bytes')
            row = {'pdf': pdf.name, 'expected': 'A B',
                   'sha256': hashlib.sha256(pdf.read_bytes()).hexdigest()}
            manifest = root / 'results.json'
            manifest.write_text(json.dumps({'cases': [row]}), encoding='utf-8')
            self.assertEqual(self.check.fixture(manifest, pdf.name), row)
            with self.assertRaises(ValueError):
                self.check.fixture(manifest, '../control.pdf')
            with self.assertRaises(ValueError):
                self.check.fixture(manifest, 'missing.pdf')
            pdf.write_bytes(b'changed bytes')
            with self.assertRaises(ValueError):
                self.check.fixture(manifest, pdf.name)
            manifest.write_text(json.dumps({'cases': [row, row]}), encoding='utf-8')
            with self.assertRaises(ValueError):
                self.check.fixture(manifest, pdf.name)


if __name__ == '__main__':
    unittest.main()
