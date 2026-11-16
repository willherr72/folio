"""Generation must reject altered inputs and runtime tables, not bless them."""
from pathlib import Path
import hashlib,json,subprocess,sys,tempfile,unittest

ROOT=Path(__file__).resolve().parents[2]
SCRIPT=ROOT/'scripts/generate-search-unicode.py'
INPUTS=ROOT/'tests/fixtures/search/unicode'

class SearchUnicodeGenerationTests(unittest.TestCase):
    def folder(self):
        parent=ROOT/'artifacts/search-unicode-generator-tests'
        parent.mkdir(parents=True,exist_ok=True)
        return Path(tempfile.mkdtemp(prefix='control-',dir=parent))

    def run_generator(self,folder,inputs=INPUTS,check=False):
        args=[sys.executable,SCRIPT,'--inputs',inputs,'--output',folder/'search-unicode.json','--license-output',folder/'UNICODE-LICENSE.txt']
        if check:args+=['--check']
        result=subprocess.run(list(map(str,args)),cwd=ROOT,capture_output=True)
        (folder/('check.log'if check else'generate.log')).write_bytes(result.stdout+result.stderr)
        return result

    def test_pinned_inputs_generate_real_full_folds_and_word_ranges_reproducibly(self):
        folder=self.folder();result=self.run_generator(folder)
        self.assertEqual(result.returncode,0,result.stderr.decode(errors='replace'))
        data=json.loads((folder/'search-unicode.json').read_text())
        folds=dict(data['folds']);self.assertEqual(folds[0xdf],'ss');self.assertEqual(folds[0x3c2],'σ');self.assertEqual(folds[0x10400],'𐐨')
        def word(cp):return any(start<=cp<=end for start,end in data['wordRanges'])
        for cp in [ord('A'),ord('_'),0x301,0x200c,0x200d,0x4e00,0x9fff]:self.assertTrue(word(cp),hex(cp))
        for cp in [ord(' '),ord('-'),ord("'"),0x2019,0x1f600]:self.assertFalse(word(cp),hex(cp))
        self.assertEqual(self.run_generator(folder,check=True).returncode,0)

    def test_changed_source_and_rewritten_provenance_cannot_authorize_new_data(self):
        folder=self.folder();inputs=folder/'inputs';inputs.mkdir()
        for source in INPUTS.iterdir():
            if source.is_file():(inputs/source.name).write_bytes(source.read_bytes())
        path=inputs/'CaseFolding-17.0.0.txt';path.write_bytes(path.read_bytes().replace(b'0041; C; 0061;',b'0041; C; 0062;',1))
        provenance=json.loads((inputs/'provenance.json').read_text());provenance['sources'][path.name]['sha256']=hashlib.sha256(path.read_bytes()).hexdigest()
        (inputs/'provenance.json').write_text(json.dumps(provenance))
        self.assertNotEqual(self.run_generator(folder,inputs).returncode,0)
        self.assertFalse((folder/'search-unicode.json').exists())

    def test_check_refuses_a_changed_runtime_table_without_rewriting_it(self):
        folder=self.folder();result=self.run_generator(folder)
        self.assertEqual(result.returncode,0,result.stderr.decode(errors='replace'))
        path=folder/'search-unicode.json';data=json.loads(path.read_text());data['folds'][0][1]='wrong'
        changed=json.dumps(data).encode();path.write_bytes(changed)
        self.assertNotEqual(self.run_generator(folder,check=True).returncode,0)
        self.assertEqual(path.read_bytes(),changed)

if __name__=='__main__':unittest.main()
