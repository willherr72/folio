"""Regressions for complete native test fixture identity, including shared files."""
from pathlib import Path
import importlib.util
import tempfile
import unittest

SCRIPT=Path(__file__).with_name('native-test-inputs.py')
spec=importlib.util.spec_from_file_location('native_test_inputs',SCRIPT)
inputs=importlib.util.module_from_spec(spec)
spec.loader.exec_module(inputs)

class NativeInputTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(prefix='Folio-native-inputs-test-')
        self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name).resolve()
        for name in ('src-tauri/tests','tests/fixtures','examples','public/ocr'):
            (self.root/name).mkdir(parents=True)
        self.native=self.root/'src-tauri/tests/example.rs';self.native.write_bytes(b'test source')
        self.shared=self.root/'tests/fixtures/shared-font.ttf';self.shared.write_bytes(b'font fixture')
        (self.root/'examples/sample.pdf').write_bytes(b'example fixture')
        (self.root/'public/ocr/manifest.json').write_bytes(b'ocr fixture')
        self.baseline=inputs.capture(self.root)

    def test_unchanged_inputs_include_shared_fixtures_and_accept(self):
        self.assertEqual(set(self.baseline),{'src-tauri/tests/example.rs','tests/fixtures/shared-font.ttf','examples/sample.pdf','public/ocr/manifest.json'})
        inputs.verify(self.root,self.baseline)

    def test_changed_shared_font_is_rejected(self):
        self.shared.write_bytes(b'changed font')
        with self.assertRaises(ValueError):inputs.verify(self.root,self.baseline)

    def test_missing_shared_font_is_rejected(self):
        self.shared.unlink()
        with self.assertRaises(ValueError):inputs.verify(self.root,self.baseline)

    def test_new_fixture_is_rejected(self):
        (self.root/'tests/fixtures/added.pdf').write_bytes(b'new fixture')
        with self.assertRaises(ValueError):inputs.verify(self.root,self.baseline)

    def test_changed_native_test_is_rejected(self):
        self.native.write_bytes(b'changed test source')
        with self.assertRaises(ValueError):inputs.verify(self.root,self.baseline)

    def test_missing_required_directory_fails_closed(self):
        self.shared.unlink();self.shared.parent.rmdir()
        with self.assertRaises(ValueError):inputs.capture(self.root)

    def test_file_outside_test_inputs_does_not_change_identity(self):
        (self.root/'unrelated.log').write_bytes(b'build output')
        inputs.verify(self.root,self.baseline)

if __name__=='__main__':unittest.main()
