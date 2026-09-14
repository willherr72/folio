import importlib.util
from pathlib import Path
import unittest
from copy import deepcopy

spec = importlib.util.spec_from_file_location('inspect_text_area', Path(__file__).with_name('inspect-text-area.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class InspectorTests(unittest.TestCase):
    def test_soft_wrap_is_not_exact_logical_copy(self):
        result = module.reader_text_result('Hello world', ['Hello', 'world'], 'Hello\nworld\n')
        self.assertFalse(result['exactLogicalText'])
        self.assertTrue(result['paintedLineText'])
        self.assertEqual(result['actualText'], 'Hello\nworld\n')

    def test_changed_or_dropped_characters_fail_line_match(self):
        for actual in ['Hello\nwold\n', 'Hello\n', 'Helloworld', 'Hello \nworld\n']:
            self.assertFalse(module.reader_text_result('Hello world', ['Hello', 'world'], actual)['paintedLineText'])

    def test_exact_copy_is_separate_from_line_structure(self):
        result = module.reader_text_result('Hello world', ['Hello', 'world'], 'Hello world')
        self.assertTrue(result['exactLogicalText'])
        self.assertFalse(result['paintedLineText'])

    def test_line_end_conventions_only_affect_line_structure(self):
        result = module.reader_text_result('Hello\nworld', ['Hello', 'world'], 'Hello\r\nworld')
        self.assertFalse(result['exactLogicalText'])
        self.assertTrue(result['paintedLineText'])

    def test_other_unicode_separators_are_not_normalized(self):
        result = module.reader_text_result('Hello world', ['Hello', 'world'], 'Hello\u2028world')
        self.assertFalse(result['paintedLineText'])

    def fixture(self):
        def span(a, b, c, d):
            return {'utf8Start':a,'utf8End':b,'utf16Start':c,'utf16End':d}
        request = {'text':'café  \r\nA'}
        lines = [
            {'source':span(0,5,0,4),'delimiter':span(5,9,4,8),'breakKind':'hard','shaped':{'text':'café','fontId':'font'}},
            {'source':span(9,10,8,9),'delimiter':span(10,10,9,9),'breakKind':'end','shaped':{'text':'A','fontId':'font'}}]
        return {'name':'test','request':request,'paintedLines':['café','A'],
                'layout':{'request':request,'fontId':'font','lines':lines,'overflow':{'horizontal':False,'vertical':False},'canExport':True}}

    def test_trailing_space_ranges_retain_exact_unicode(self):
        self.assertTrue(module.validate_layout(self.fixture()))

    def test_bad_ranges_missing_shapes_and_unrelated_painted_lines_fail(self):
        for mutate in [lambda c: c['layout']['lines'][0]['source'].update(utf16End=5),
                       lambda c: c['layout']['lines'][0].update(shaped=None),
                       lambda c: c.update(paintedLines=['changed','A']),
                       lambda c: c['layout']['lines'][0]['delimiter'].update(utf8End=10)]:
            case = deepcopy(self.fixture()); mutate(case)
            with self.assertRaises(AssertionError):
                module.validate_layout(case)

    def test_empty_layout_does_not_discard_final_row(self):
        case = self.fixture(); case['request']['text'] = ''; case['layout']['lines'] = []; case['paintedLines'] = []
        with self.assertRaises(AssertionError):
            module.validate_layout(case)

    def test_trailing_hard_break_must_have_final_empty_row(self):
        case = self.fixture(); case['request']['text'] = 'café  \r\n'; case['layout']['lines'].pop(); case['paintedLines'].pop()
        with self.assertRaises(AssertionError):
            module.validate_layout(case)

    def test_terminal_eol_diagnostic_does_not_hide_soft_wrap(self):
        single = module.reader_text_result('Hello world', ['Hello world'], 'Hello world\n')
        self.assertTrue(single['logicalTextIgnoringOneTerminalEol'])
        self.assertFalse(single['exactLogicalText'])
        wrapped = module.reader_text_result('Hello world', ['Hello', 'world'], 'Hello\nworld\n')
        self.assertFalse(wrapped['logicalTextIgnoringOneTerminalEol'])

    def test_geometry_ignores_only_generated_type3_object_labels(self):
        a = {'blocks':[{'lines':[{'spans':[{'font':'Type3 (28 0 R)','bbox':[1.,2.,3.,4.],'chars':[{'c':'A','origin':[1.,2.]}]}]}]}]}
        b = deepcopy(a); b['blocks'][0]['lines'][0]['spans'][0]['font'] = 'Type3 (18 0 R)'
        self.assertEqual(module.geometry_without_resource_labels(a), module.geometry_without_resource_labels(b))
        self.assertNotEqual(a, b)
        b['blocks'][0]['lines'][0]['spans'][0]['bbox'][0] = 1.01
        self.assertNotEqual(module.geometry_without_resource_labels(a), module.geometry_without_resource_labels(b))
        b = deepcopy(a); b['blocks'][0]['lines'][0]['spans'][0]['font'] = 'OtherFont'
        self.assertNotEqual(module.geometry_without_resource_labels(a), module.geometry_without_resource_labels(b))

    def test_empty_matrix_cannot_pass(self):
        with self.assertRaises(ValueError):
            module.validate_case_names([], 0)
        with self.assertRaises(ValueError):
            module.validate_case_names([{'name':'x'}, {'name':'x'}], 2)
        with self.assertRaises(ValueError):
            module.validate_case_names([{'name':'x'}], 2)

if __name__ == '__main__':
    unittest.main()
