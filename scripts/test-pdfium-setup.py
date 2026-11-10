"""Regression checks for setup retaining the corrected renderer and failing closed."""
from pathlib import Path
import hashlib
import json
import shutil
import subprocess
import tempfile
import unittest

REPO = Path(__file__).resolve().parent.parent


class SetupTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='folio-pdfium-setup-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / 'scripts').mkdir()
        shutil.copyfile(REPO / 'scripts/fetch-pdfium.ps1', self.root / 'scripts/fetch-pdfium.ps1')
        lock_path = self.root / 'scripts/pdfium-source-lock.json'
        shutil.copyfile(REPO / 'scripts/pdfium-source-lock.json', lock_path)
        lock = json.loads(lock_path.read_text())
        patch = self.root / 'patches/pdfium/form-text-state-inheritance.patch'
        patch.parent.mkdir(parents=True)
        shutil.copyfile(REPO / 'patches/pdfium/form-text-state-inheritance.patch', patch)
        self.advance_patch = self.root / 'patches/pdfium/fractional-horizontal-advances.patch'
        shutil.copyfile(REPO / 'patches/pdfium/fractional-horizontal-advances.patch', self.advance_patch)
        self.color_patch = self.root / 'patches/pdfium/ordered-default-colors.patch'
        shutil.copyfile(REPO / 'patches/pdfium/ordered-default-colors.patch', self.color_patch)
        self.form_font_patch = self.root / 'patches/pdfium/embedded-form-font-preference.patch'
        shutil.copyfile(REPO / 'patches/pdfium/embedded-form-font-preference.patch', self.form_font_patch)
        self.resource = self.root / 'src-tauri/resources/pdfium'
        self.resource.mkdir(parents=True)
        self.dll = self.resource / 'pdfium.dll'
        self.dll.write_bytes(b'Owned fixture; never loaded as a DLL.')
        self.metadata = dict(variant=lock['variant'],
                             sourceCommit='f91ca5a72358bb0b00b4da9481b21fe668157614',
                             patchSha256=hashlib.sha256(patch.read_bytes()).hexdigest(),
                             dllSha256=hashlib.sha256(self.dll.read_bytes()).hexdigest())
        self.metadata.update(sourceLockSha256=hashlib.sha256(lock_path.read_bytes()).hexdigest(),
                             advancePatchSha256=hashlib.sha256(self.advance_patch.read_bytes()).hexdigest(),
                             colorPatchSha256=hashlib.sha256(self.color_patch.read_bytes()).hexdigest(),
                             formFontPatchSha256=hashlib.sha256(self.form_font_patch.read_bytes()).hexdigest(),
                             buildCommit=lock['repositories']['build']['commit'],
                             distributorCommit=lock['repositories']['distributor']['commit'],
                             depotToolsCommit=lock['repositories']['depot_tools']['commit'])

    def run_setup(self):
        return subprocess.run([shutil.which('pwsh') or 'powershell.exe', '-NoProfile', '-File',
                               str(self.root / 'scripts/fetch-pdfium.ps1')],
                              capture_output=True, text=True, timeout=15)

    def register(self):
        (self.resource / 'provenance.json').write_text(json.dumps(self.metadata))

    def test_valid_dependency_is_preserved(self):
        self.register()
        before = self.dll.read_bytes()
        result = self.run_setup()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.dll.read_bytes(), before)

    def test_tampered_dependency_is_preserved_and_rejected(self):
        self.register()
        self.dll.write_bytes(b'Changed fixture')
        result = self.run_setup()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.dll.read_bytes(), b'Changed fixture')

    def test_changed_recipe_is_rejected(self):
        self.metadata['sourceLockSha256'] = '0' * 64
        self.register()
        result = self.run_setup()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('pinned source recipe', result.stderr)

    def test_changed_advance_patch_is_rejected(self):
        self.register()
        self.advance_patch.write_bytes(b'changed advance patch')
        self.assertNotEqual(self.run_setup().returncode, 0)

    def test_missing_advance_provenance_is_rejected(self):
        self.metadata.pop('advancePatchSha256')
        self.register()
        self.assertNotEqual(self.run_setup().returncode, 0)

    def test_changed_color_patch_is_rejected(self):
        self.register()
        self.color_patch.write_bytes(b'changed color patch')
        self.assertNotEqual(self.run_setup().returncode, 0)

    def test_missing_color_provenance_is_rejected(self):
        self.metadata.pop('colorPatchSha256')
        self.register()
        self.assertNotEqual(self.run_setup().returncode, 0)

    def test_changed_form_font_patch_is_rejected(self):
        self.register()
        self.form_font_patch.write_bytes(b'changed form font patch')
        self.assertNotEqual(self.run_setup().returncode, 0)

    def test_missing_form_font_provenance_is_rejected(self):
        self.metadata.pop('formFontPatchSha256')
        self.register()
        self.assertNotEqual(self.run_setup().returncode, 0)

    def test_missing_registration_never_downloads_baseline(self):
        before = self.dll.read_bytes()
        result = self.run_setup()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('build-pdfium.ps1', result.stderr)
        self.assertEqual(self.dll.read_bytes(), before)
        self.assertFalse((self.root / 'artifacts/dependencies').exists())

    def test_old_registration_never_downloads_baseline(self):
        self.metadata.pop('variant')
        self.register()
        result = self.run_setup()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('build-pdfium.ps1', result.stderr)
        self.assertFalse((self.root / 'artifacts/dependencies').exists())


if __name__ == '__main__':
    unittest.main()
