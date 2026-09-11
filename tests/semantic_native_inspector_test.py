"""Reject broken semantic font definitions even when dictionary counts agree."""
import hashlib
import copy
import importlib.util
from pathlib import Path
import tempfile
import unittest
import sys

sys.dont_write_bytecode = True

from pypdf import PdfWriter
from pypdf.generic import ArrayObject, ContentStream, DecodedStreamObject, DictionaryObject, NameObject, NumberObject, FloatObject

SCRIPT = Path(__file__).parents[1] / "scripts/inspect-semantic-native.py"
spec = importlib.util.spec_from_file_location("semantic_inspector", SCRIPT)
inspector = importlib.util.module_from_spec(spec)
spec.loader.exec_module(inspector)


def dictionary(**values):
    return DictionaryObject({NameObject("/" + key): value for key, value in values.items()})


def fixture(path, mutation=None, banked=False, content=None):
    writer = PdfWriter()
    def stream(data):
        result = DecodedStreamObject()
        result.set_data(data)
        return writer._add_object(result)
    page = writer.add_blank_page(width=300, height=160)
    procs = dictionary(g1=stream(b"500 0 0 0 500 700 d1"), g2=stream(b"500 0 0 0 500 700 d1"))
    semantic = dictionary(
        Type=NameObject("/Font"), Subtype=NameObject("/Type3"),
        FontBBox=ArrayObject([NumberObject(v) for v in [0, 0, 500, 700]]),
        FontMatrix=ArrayObject([FloatObject(v) for v in [.001, 0, 0, .001, 0, 0]]),
        FirstChar=NumberObject(1), LastChar=NumberObject(2),
        Widths=ArrayObject([NumberObject(500), NumberObject(500)]),
        Encoding=dictionary(Differences=ArrayObject([NumberObject(1), NameObject("/g1"), NameObject("/g2")])),
        CharProcs=procs, Resources=dictionary(),
        ToUnicode=stream(b"begincmap 1 begincodespacerange <00> <FF> endcodespacerange\n"
                         b"2 beginbfchar\n<01> <0041>\n<02> <0042>\nendbfchar endcmap"))
    if mutation:
        mutation(semantic)
    original_data = b"unused original font bytes"
    original = dictionary(Subtype=NameObject("/Type0"), DescendantFonts=ArrayObject([
        dictionary(FontDescriptor=dictionary(FontFile2=stream(original_data)))]))
    page[NameObject("/Resources")] = dictionary(Font=dictionary(S=semantic, Original=original))
    if banked:
        second = copy.deepcopy(semantic)
        second[NameObject("/ToUnicode")] = stream(semantic["/ToUnicode"].get_data().replace(b"0041", b"0043").replace(b"0042", b"D835DC34"))
        page["/Resources"]["/Font"][NameObject("/S1")] = second
    page[NameObject("/Contents")] = stream(content or b"0 0 20 20 re f BT /S 24 Tf 1 0 0 1 48 48 Tm [<01> <02> <01>] TJ ET")
    writer.write(path)
    return hashlib.sha256(original_data).hexdigest()


class SemanticNativeInspectorTests(unittest.TestCase):
    def inspect(self, mutation=None, expected_visual="ABA", **options):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "candidate.pdf"
            font_hash = fixture(path, mutation, **options)
            return inspector.inspect_pdf(path, font_hash, expected_visual)["structure"]

    def test_reused_codes_remain_valid(self):
        result = self.inspect()
        self.assertTrue(result["passes"])
        self.assertEqual(result["semantic_codes"][0]["scalar_use_count"], 3)
        self.assertEqual(result["semantic_codes"][0]["unique_code_count"], 2)

    def test_banked_codes_decode_in_emission_order_and_reuse_first_bank(self):
        result = self.inspect(banked=True, expected_visual="ABC\U0001d434A", content=
            b"0 0 20 20 re f BT /S 24 Tf 1 0 0 1 48 48 Tm [<0102> -2] TJ /S1 24 Tf [<0102> 2] TJ /S 24 Tf [<01>] TJ ET")
        self.assertTrue(result["passes"], result)
        self.assertEqual([c["scalar_use_count"] for c in result["semantic_codes"]], [3, 2])
        self.assertEqual(result["semantic_text"]["decoded_visual_text"], "ABC\U0001d434A")
        self.assertEqual(result["text_objects"], 1)
        self.assertTrue(result["original_font_matches"])
        self.assertTrue(result["original_font_unused"])

    def test_banked_wrong_font_mapping_and_missing_occurrence_fail(self):
        content = b"0 0 20 20 re f BT /S 24 Tf 1 0 0 1 48 48 Tm [<0102>] TJ /S1 24 Tf [<0102>] TJ /S 24 Tf [<01>] TJ ET"
        for wrong in (content.replace(b"/S1 24", b"/S 24"), content.replace(b"[<01>] TJ", b"[] TJ")):
            with self.subTest(content=wrong):
                self.assertFalse(self.inspect(banked=True, expected_visual="ABC\U0001d434A", content=wrong)["passes"])

    def test_malformed_text_operations_fail_closed(self):
        prefix = b"0 0 20 20 re f BT /S 24 Tf 1 0 0 1 48 48 Tm "
        for content in (
            prefix + b"[<010201>] TJ",  # Missing ET.
            prefix + b"[<010201>] TJ ET ET",
            prefix + b"ET [<010201>] TJ",  # Draw outside BT/ET.
            prefix + b"[<010201>] 7 TJ ET",  # Extra operand.
            prefix + b"[<010201>] TJ 1 2 Unknown ET",
            prefix + b"[<01>] TJ [<0201>] TJ ET",  # Split arrays without a bank switch.
            prefix + b"[<010201>] TJ /S 24 Tf ET",  # Trailing unused selection.
        ):
            with self.subTest(content=content):
                with tempfile.TemporaryDirectory() as directory:
                    path = Path(directory) / "candidate.pdf"
                    fixture(path, content=content)
                    reader = inspector.PdfReader(path)
                    page = reader.pages[0]
                    _, result = inspector.semantic_text({"/S": page["/Resources"]["/Font"]["/S"]},
                        ContentStream(page.get_contents(), reader).operations, reader, "ABA")
                    self.assertFalse(result["passes"])

    def test_supplementary_mapping_is_one_scalar_per_byte(self):
        def mutate(font):
            cmap = font["/ToUnicode"]
            cmap.set_data(cmap.get_data().replace(b"0042", b"D835DC34"))
        result = self.inspect(mutate, "A\U0001d434A")
        self.assertTrue(result["passes"])
        self.assertEqual(result["semantic_codes"][0]["scalar_use_count"], 3)

    def test_wrong_unicode_mapping_with_same_counts_fails(self):
        def mutate(font):
            cmap = font["/ToUnicode"]
            cmap.set_data(cmap.get_data().replace(b"0042", b"0041"))
        self.assertFalse(self.inspect(mutate)["passes"])

    def test_unpaired_surrogate_mapping_fails(self):
        def mutate(font):
            cmap = font["/ToUnicode"]
            cmap.set_data(cmap.get_data().replace(b"0042", b"D835"))
        self.assertFalse(self.inspect(mutate)["passes"])

    def test_duplicate_cmap_code_fails(self):
        def mutate(font):
            cmap = font["/ToUnicode"]
            cmap.set_data(cmap.get_data().replace(b"<02>", b"<01>"))
        self.assertFalse(self.inspect(mutate)["passes"])

    def test_missing_referenced_charproc_fails(self):
        result = self.inspect(lambda font: font["/CharProcs"].pop("/g2"))
        self.assertFalse(result["passes"])

    def test_width_disagrees_with_charproc_fails(self):
        def mutate(font):
            font["/Widths"][1] = NumberObject(700)
        self.assertFalse(self.inspect(mutate)["passes"])

    def test_missing_cmap_definition_fails(self):
        def mutate(font):
            font["/ToUnicode"].set_data(b"begincmap 1 begincodespacerange <00> <FF> endcodespacerange 1 beginbfchar <01> <0041> endbfchar endcmap")
        self.assertFalse(self.inspect(mutate)["passes"])

    def test_duplicate_encoding_alias_fails(self):
        def mutate(font):
            font["/Encoding"]["/Differences"][2] = NameObject("/g1")
        self.assertFalse(self.inspect(mutate)["passes"])


if __name__ == "__main__":
    unittest.main()
