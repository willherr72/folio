"""Exercise the real verifier against isolated package and ZIP bytes."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import warnings
import zipfile

SCRIPT=Path(__file__).resolve().parents[1]/'verify-portable.py'
def digest(data):return hashlib.sha256(data).hexdigest()

class PortableVerificationTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.cwd=Path(self.temp.name);self.root=self.cwd/'extracted package'
        self.root.mkdir();self.archive=self.cwd/'bundle.zip'
        files={'Folio.exe':b'desktop executable','folio-cli.exe':b'console executable',
               'resources/pdfium/pdfium.dll':b'one shared renderer',
               'docs/automation.md':b'# Automation guide',
               'licenses/ocr/tessdata_best/LICENSE':b'L'*1200,
               'licenses/ocr/asset-provenance.json':b'{"language":"test"}',
               'licenses/dependencies.json':b'[{"name":"tessdata_best"}]'}
        for name,data in files.items():
            path=self.root/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
        source={'src/example.rs':digest(b'source'),'src-tauri/resources/pdfium/pdfium.dll':digest(files['resources/pdfium/pdfium.dll'])}
        fingerprint=digest(json.dumps(source,sort_keys=True,separators=(',',':')).encode())
        self.write_json('build-source-fingerprint.json',{'files':source,'fingerprint':fingerprint})
        self.write_json('build-info.json',{'version':'0.22.0-dev.1','configuration':'release','sourceFingerprint':fingerprint,
            'executableSha256':digest(files['Folio.exe']),'cliExecutableSha256':digest(files['folio-cli.exe']),
            'pdfiumSha256':digest(files['resources/pdfium/pdfium.dll']),
            'ocrAssetManifestSha256':digest(files['licenses/ocr/asset-provenance.json'])})
        self.pack()

    def write_json(self,name,value):
        (self.root/name).write_text(json.dumps(value),encoding='utf-8')

    def pack(self,duplicate=False):
        with zipfile.ZipFile(self.archive,'w') as z:
            for p in self.root.rglob('*'):
                if p.is_file():z.write(p,p.relative_to(self.root).as_posix())
            if duplicate:
                with warnings.catch_warnings():
                    warnings.simplefilter('ignore');z.write(self.root/'Folio.exe','Folio.exe')
        self.archive.with_suffix('.zip.sha256').write_text(digest(self.archive.read_bytes())+'  bundle.zip\n',encoding='utf-8')

    def run_verifier(self,*args):
        return subprocess.run([sys.executable,str(SCRIPT),'--package',str(self.root),'--archive',str(self.archive),*args],cwd=self.cwd,capture_output=True,text=True,timeout=30)

    def refuses(self,reason):
        result=self.run_verifier();self.assertNotEqual(result.returncode,0);self.assertIn(reason,result.stderr)

    def test_explicit_extracted_package_outside_checkout(self):
        result=self.run_verifier();self.assertEqual(result.returncode,0,result.stderr)
        report=json.loads(result.stdout);self.assertEqual(report['cli_executable_sha256'],digest(b'console executable'))

    def test_missing_cli(self):
        (self.root/'folio-cli.exe').unlink();self.pack();self.refuses('folio-cli.exe')

    def test_changed_cli_even_when_archive_matches(self):
        (self.root/'folio-cli.exe').write_bytes(b'stale executable');self.pack();self.refuses('folio-cli.exe hash')

    def test_missing_guide(self):
        (self.root/'docs/automation.md').unlink();self.pack();self.refuses('docs/automation.md')

    def test_changed_manifest_fingerprint(self):
        source=json.loads((self.root/'build-source-fingerprint.json').read_text());source['files']['src/example.rs']=digest(b'changed')
        self.write_json('build-source-fingerprint.json',source);self.pack();self.refuses('source fingerprint')

    def test_changed_renderer(self):
        (self.root/'resources/pdfium/pdfium.dll').write_bytes(b'wrong renderer');self.pack();self.refuses('pdfium.dll hash')

    def test_archive_duplicate(self):
        self.pack(duplicate=True);self.refuses('Duplicate archive')

    def test_changed_ocr_provenance(self):
        self.write_json('licenses/ocr/asset-provenance.json',{'language':'wrong'})
        self.pack();self.refuses('asset-provenance.json hash')

    def test_archive_and_package_differ(self):
        (self.root/'docs/automation.md').write_bytes(b'changed documentation');self.refuses('Archive entry differs')

    def test_archive_extra_path(self):
        with zipfile.ZipFile(self.archive,'a') as z:z.writestr('../outside.txt','no extraction')
        self.archive.with_suffix('.zip.sha256').write_text(digest(self.archive.read_bytes()),encoding='utf-8')
        self.refuses('Noncanonical archive path');self.assertFalse((self.cwd.parent/'outside.txt').exists())

    def test_unexpected_regular_file(self):
        with zipfile.ZipFile(self.archive,'a') as z:z.writestr('extra.txt','not in package')
        self.archive.with_suffix('.zip.sha256').write_text(digest(self.archive.read_bytes()),encoding='utf-8')
        self.refuses('Archive and package differ')

    def test_existing_report_is_preserved(self):
        report=self.cwd/'report.json';report.write_bytes(b'keep original evidence')
        result=self.run_verifier('--report',str(report));self.assertNotEqual(result.returncode,0)
        self.assertIn('File exists',result.stderr)
        self.assertEqual(report.read_bytes(),b'keep original evidence')

    def test_new_report_matches_stdout(self):
        report=self.cwd/'report.json';result=self.run_verifier('--report',str(report))
        self.assertEqual(result.returncode,0,result.stderr);self.assertEqual(json.loads(report.read_text()),json.loads(result.stdout))

    def test_report_cannot_invalidate_the_package_it_just_verified(self):
        report=self.root/'verification.json';result=self.run_verifier('--report',str(report))
        self.assertNotEqual(result.returncode,0)
        self.assertIn('Report must be outside the package',result.stderr)
        self.assertFalse(report.exists())

    def add_directories(self,names):
        with zipfile.ZipFile(self.archive,'a') as z:
            with warnings.catch_warnings():
                warnings.simplefilter('ignore')
                for name in names:z.writestr(name,b'')
        self.archive.with_suffix('.zip.sha256').write_text(digest(self.archive.read_bytes()),encoding='utf-8')

    def test_directory_file_collision(self):
        self.add_directories(['Folio.exe/']);self.refuses('Archive path collision')

    def test_duplicate_directories(self):
        self.add_directories(['docs/','docs/']);self.refuses('Duplicate archive')

    def test_unexpected_directory(self):
        self.add_directories(['unexpected/']);self.refuses('Archive directory differs')

    def test_unsafe_directory(self):
        self.add_directories(['../outside/']);self.refuses('Noncanonical archive path')

    def test_empty_directory_roundtrip(self):
        (self.root/'empty').mkdir();self.add_directories(['empty/'])
        result=self.run_verifier();self.assertEqual(result.returncode,0,result.stderr)

    def test_missing_empty_directory(self):
        (self.root/'empty').mkdir();self.refuses('Empty package directory is absent')

    @unittest.skipUnless(sys.platform=='win32','PowerShell packaging preflight is Windows-only')
    def test_crlf_cargo_manifest_refuses_before_building(self):
        project=self.cwd/'source';(project/'scripts').mkdir(parents=True);(project/'src-tauri').mkdir()
        builder=project/'scripts/build-portable.ps1'
        builder.write_bytes((SCRIPT.parent/'build-portable.ps1').read_bytes())
        for name in ['package.json','src-tauri/tauri.conf.json']:
            (project/name).write_text('{"version":"0.22.0-dev.1"}',encoding='utf-8')
        manifest=b'[package]\r\nname = "folio"\r\nversion = "0.22.0-dev.1"\r\n'
        (project/'src-tauri/Cargo.toml').write_bytes(manifest)
        result=subprocess.run(['pwsh','-NoProfile','-File',str(builder)],cwd=self.cwd,capture_output=True,text=True,timeout=30)
        self.assertNotEqual(result.returncode,0);self.assertIn('Cargo.toml must use LF line endings',result.stderr)
        self.assertEqual((project/'src-tauri/Cargo.toml').read_bytes(),manifest)
        self.assertFalse((project/'artifacts').exists())

if __name__=='__main__':unittest.main()
