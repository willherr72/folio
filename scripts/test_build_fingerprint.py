"""A path-patched CFF parser must be bound by the same product source guard."""
from pathlib import Path
import ast,json,shutil,subprocess,sys,tempfile,unittest

SCRIPT=Path(__file__).with_name('build-fingerprint.py')
PARSER='src-tauri/vendor/ttf-parser-0.25.1/src/tables/cff/folio.rs'

class VendoredParserFingerprintTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name);self.script=self.root/'scripts/build-fingerprint.py'
        self.script.parent.mkdir();shutil.copy2(SCRIPT,self.script)
        # The copied driver resolves its own repository root. Supply its small
        # mandatory metadata files without copying the real product or renderer.
        for node in ast.walk(ast.parse(SCRIPT.read_text(encoding='utf-8'))):
            if isinstance(node,ast.Constant)and isinstance(node.value,str):
                path=Path(node.value)
                if not path.is_absolute()and path.suffix in {'.json','.toml','.rs','.patch','.dll','.lock'}:
                    target=self.root/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(b'fixture')
        self.parser=self.root/PARSER;self.parser.parent.mkdir(parents=True);self.parser.write_text('pub fn proof() {}\n')
        self.evidence=self.root/'evidence';self.evidence.mkdir()
        self.call('write',success=True)

    def call(self,mode,success):
        result=subprocess.run([sys.executable,self.script,mode,self.evidence],capture_output=True)
        self.assertEqual(result.returncode==0,success,result.stdout.decode()+result.stderr.decode())

    def test_manifest_includes_the_compiled_parser(self):
        manifest=json.loads((self.evidence/'build-source-fingerprint.json').read_text())
        self.assertIn(PARSER,manifest['files'])

    def test_changed_parser_bytes_are_rejected(self):
        self.parser.write_text('pub fn proof() { panic!("changed") }\n')
        self.call('check',success=False)

    def test_new_parser_module_membership_is_rejected(self):
        self.parser.with_name('unreviewed.rs').write_text('pub fn extra() {}\n')
        self.call('check',success=False)

if __name__=='__main__':unittest.main()
