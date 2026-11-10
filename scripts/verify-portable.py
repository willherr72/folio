"""Verify a combined portable package without extracting archive entries.

Checks integrity against the local build records, not publisher authenticity.
Explicit paths work from any current directory; importing has no side effects.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import sys
import zipfile

REPO=Path(__file__).resolve().parent.parent

def sha256(path):
    with path.open('rb') as source:return hashlib.file_digest(source,'sha256').hexdigest()

def read_json(path):return json.loads(path.read_text(encoding='utf-8-sig'))

def require(condition,message):
    if not condition:raise ValueError(message)

def verify_package(root: Path,archive: Path,expected_version=None):
    root=Path(root);archive=Path(archive)
    for name in ['Folio.exe','folio-cli.exe','resources/pdfium/pdfium.dll','docs/automation.md',
                 'build-info.json','build-source-fingerprint.json','licenses/dependencies.json',
                 'licenses/ocr/asset-provenance.json','licenses/ocr/tessdata_best/LICENSE']:
        require((root/name).is_file(),f'Missing required package file: {name}')
    info=read_json(root/'build-info.json');version=info['version']
    require(isinstance(version,str) and re.fullmatch(r'\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?',version),'Invalid package version')
    require(expected_version is None or version==expected_version,'Package version differs')
    require(info['configuration']=='release','Package is not a release configuration build')
    for name,key in [('Folio.exe','executableSha256'),('folio-cli.exe','cliExecutableSha256'),
                     ('resources/pdfium/pdfium.dll','pdfiumSha256'),('licenses/ocr/asset-provenance.json','ocrAssetManifestSha256')]:
        require(sha256(root/name)==info[key],f'{name} hash differs from build record')
    manifest=read_json(root/'build-source-fingerprint.json')
    fingerprint=hashlib.sha256(json.dumps(manifest['files'],sort_keys=True,separators=(',',':')).encode()).hexdigest()
    require(fingerprint==manifest['fingerprint']==info['sourceFingerprint'],'Package source fingerprint differs')
    require(manifest['files'].get('src-tauri/resources/pdfium/pdfium.dll')==info['pdfiumSha256'],'Renderer differs from source fingerprint')
    digest=sha256(archive)
    checksum=Path(str(archive)+'.sha256').read_text(encoding='utf-8-sig').split()
    require(bool(checksum) and checksum[0]==digest,'Archive checksum differs')
    expected={p.relative_to(root).as_posix():p for p in root.rglob('*') if p.is_file()}
    directories={p.relative_to(root).as_posix():p for p in root.rglob('*') if p.is_dir()}
    with zipfile.ZipFile(archive) as bundle:
        all_entries=bundle.infolist()
        all_names=[entry.filename for entry in all_entries]
        require(len(all_names)==len(set(all_names)),'Duplicate archive names')
        paths=set();archived_directories=set()
        for entry in all_entries:
            name=entry.filename;path=PurePosixPath(name)
            canonical=path.as_posix()+('/' if entry.is_dir() else '')
            require(name==canonical and not path.is_absolute() and '..' not in path.parts
                    and '\\' not in name and ':' not in name,'Noncanonical archive path: '+name)
            key=name.rstrip('/')
            require(key not in paths,'Archive path collision: '+key);paths.add(key)
            if entry.is_dir():
                require(key in directories and entry.file_size==0,'Archive directory differs: '+key)
                archived_directories.add(key)
        for name,path in directories.items():
            if not any(path.iterdir()):require(name in archived_directories,'Empty package directory is absent: '+name)
        entries=[entry for entry in all_entries if not entry.is_dir()]
        names=[entry.filename for entry in entries]
        require(set(names)==set(expected),'Archive and package differ')
        for entry in entries:
            path=expected[entry.filename]
            require(entry.file_size==path.stat().st_size,f'Archive entry differs: {entry.filename}')
            with bundle.open(entry) as source:archive_hash=hashlib.file_digest(source,'sha256').hexdigest()
            require(archive_hash==sha256(path),f'Archive entry differs: {entry.filename}')
    assets=read_json(root/'licenses/ocr/asset-provenance.json')
    require(isinstance(assets,dict),'Invalid OCR asset provenance')
    require((root/'licenses/ocr/tessdata_best/LICENSE').stat().st_size>1000,'OCR license is missing or truncated')
    licenses=read_json(root/'licenses/dependencies.json')
    require(any(entry.get('name')=='tessdata_best' for entry in licenses),'OCR dependency record is missing')
    return {'version':version,'package':str(root.resolve()),'archive':str(archive.resolve()),
            'archive_sha256':digest,'executable_sha256':info['executableSha256'],
            'cli_executable_sha256':info['cliExecutableSha256'],'source_fingerprint':fingerprint,
            'verified_files':len(expected),'dependency_records':len(licenses),'ocr_asset_provenance':True}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package',type=Path);parser.add_argument('--archive',type=Path)
    parser.add_argument('--report',type=Path,help='Create a new report; existing files are preserved')
    args=parser.parse_args()
    if (args.package is None)!=(args.archive is None):parser.error('--package and --archive must be supplied together')
    expected=None
    if args.package is None:
        expected=read_json(REPO/'package.json')['version']
        args.package=REPO/'artifacts'/f'Folio-v{expected}'
        args.archive=args.package.with_name(args.package.name+'-windows-x64.zip')
    if args.report:
        require(not args.report.resolve().is_relative_to(args.package.resolve()),'Report must be outside the package')
    report=verify_package(args.package,args.archive,expected)
    if args.report:
        with args.report.open('x',encoding='utf-8') as output:json.dump(report,output,indent=2);output.write('\n')
    print(json.dumps(report))

if __name__=='__main__':
    try:main()
    except (OSError,ValueError,KeyError,TypeError,zipfile.BadZipFile) as error:
        print(f'Portable verification failed: {error}',file=sys.stderr);sys.exit(1)
