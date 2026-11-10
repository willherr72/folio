"""Capture/check native test sources and shared repository fixtures.

Use alongside build-fingerprint.py, which binds product code and dependencies:
  python scripts/native-test-inputs.py write <existing-evidence-directory>
  # Run the native test command, retaining its terminal result and output.
  python scripts/native-test-inputs.py check <same-evidence-directory>

This records file membership and bytes, not external toolchain/system-font
identity or an atomic filesystem snapshot. Existing manifests are preserved.
"""
from pathlib import Path
import argparse
import hashlib
import json

ROOT=Path(__file__).resolve().parent.parent
INPUT_DIRECTORIES=('src-tauri/tests','tests/fixtures','examples','public/ocr')

def capture(root):
    files={}
    for directory in INPUT_DIRECTORIES:
        base=root/directory
        if not base.is_dir():raise ValueError('Missing input directory: '+directory)
        for path in sorted(base.rglob('*')):
            if path.is_file():
                with path.open('rb') as stream:
                    files[path.relative_to(root).as_posix()]=hashlib.file_digest(stream,'sha256').hexdigest()
    return files

def verify(root,expected):
    if capture(root)!=expected:
        raise ValueError('Native test or shared fixture membership/bytes changed')

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode',choices=('write','check'))
    parser.add_argument('directory',type=Path)
    args=parser.parse_args()
    path=args.directory/'native-test-inputs.json'
    if args.mode=='write':
        files=capture(ROOT)
        with path.open('x',encoding='utf-8') as stream:
            json.dump(files,stream,indent=2);stream.write('\n')
    else:
        verify(ROOT,json.loads(path.read_text(encoding='utf-8')))
    print('Native test and shared fixture inputs: '+args.mode+' passed')

if __name__=='__main__':main()
