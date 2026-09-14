"""Checks that the diagnostic measures real selection and writes connected tags."""
import importlib.util
import tempfile
import unittest
from pathlib import Path

import fitz
from pypdf import PdfReader

spec = importlib.util.spec_from_file_location('probe', Path(__file__).with_name('probe-tagged-selection.py'))


class ProbeTests(unittest.TestCase):
    def setUp(self):
        self.assertTrue(Path(spec.origin).exists(), 'Tagged selection probe must exist')
        self.probe = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.probe)

    def test_selection_control_has_no_extractor_terminal_newline(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'control.pdf'
            self.probe.separator.write_case(path, 'A B', False, 'scalar-cells', 0)
            result = self.probe.mupdf_selection(path)
            self.assertEqual(result['pageText'], 'A B\n')
            self.assertEqual(result['selectionLf'], 'A B')
            self.assertEqual(result['selectionCrlf'], 'A B')

    def test_tags_connect_parent_tree_page_and_exact_replacement(self):
        with tempfile.TemporaryDirectory() as directory:
            source, target = [Path(directory) / name for name in ['source.pdf', 'tagged.pdf']]
            self.probe.separator.write_case(source, 'A  B', False, 'scalar-cells', 0)
            self.probe.tag_pdf(source, target, 'A  B', 'structure-actualtext')
            pdf = PdfReader(target)
            root = pdf.trailer['/Root']
            tree = root['/StructTreeRoot']
            document = tree['/K']
            paragraph = document['/K']
            span = paragraph['/K']
            self.assertEqual(span['/ActualText'], 'A  B')
            self.assertEqual(span['/K'], 0)
            self.assertEqual(span['/Pg'].indirect_reference, pdf.pages[0].indirect_reference)
            self.assertEqual(tree['/ParentTree']['/Nums'][1][0], span.indirect_reference)
            self.assertEqual(pdf.pages[0]['/StructParents'], 0)
            self.assertIn(b'/MCID 0', pdf.pages[0].get_contents().get_data())
            with fitz.open(source) as a, fitz.open(target) as b:
                self.assertEqual(a[0].get_pixmap().samples, b[0].get_pixmap().samples)

    def test_selection_preserves_repeated_spaces_without_normalization(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'spaces.pdf'
            self.probe.separator.write_case(path, 'A  B', False, 'scalar-cells', 0)
            self.assertEqual(self.probe.mupdf_selection(path)['selectionLf'], 'A  B')


if __name__ == '__main__':
    unittest.main()
