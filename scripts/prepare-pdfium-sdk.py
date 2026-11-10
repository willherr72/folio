"""Assemble a private Windows SDK from the pinned, signature-verified packages.

Downloads/signature verification are recorded separately. This does not install
packages, edit the registry, or change the machine's selected SDK.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import stat
import zipfile
import zlib

PACKAGES = {
    'sdk-cpp-10.0.28000.2270.nupkg': '0a4887a64d1b17128f9ffd80032f20debabf943f34eea927aac00cc46cb32879',
    'sdk-cpp-x64-10.0.28000.2270.nupkg': '72076d00b16c3882bf9eab0c80e350f33d8bc65e5f29e2f0e9b66a3f2569ccb1',
    'sdk-cpp-x86-10.0.28000.2270.nupkg': '5bf7b8811cd8e7640b5475ddbbf0a873f90077622c73bc6f6c2f1d368d5d55e3',
    'sdk-debuggers-20260319.1511.0.nupkg': '875678516f9ceed4a1c8b9b106d165106fcaa38723ad7c584c47b95b231600de',
}


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def unchanged(path, entry):
    if path.stat().st_size != entry.file_size:
        return False
    checksum = 0
    with path.open('rb') as stream:
        while chunk := stream.read(1024 * 1024):
            checksum = zlib.crc32(chunk, checksum)
    return checksum == entry.CRC


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    args = parser.parse_args()
    root = args.root.resolve()
    output = (root / 'sdk').resolve()
    for name, expected in PACKAGES.items():
        if sha256(root / name) != expected:
            raise SystemExit(f'Package fingerprint mismatch: {name}')
    output.mkdir(exist_ok=True)
    count = 0
    for name in PACKAGES:
        with zipfile.ZipFile(root / name) as package:
            for entry in package.infolist():
                path = PurePosixPath(entry.filename)
                is_debugger = name.startswith('sdk-debuggers-')
                prefix = ('content', 'amd64') if is_debugger else ('c',)
                if path.parts[:len(prefix)] != prefix or entry.is_dir():
                    continue
                if '..' in path.parts or stat.S_ISLNK(entry.external_attr >> 16):
                    raise SystemExit(f'Unsafe package entry: {entry.filename}')
                relative = path.parts[len(prefix):]
                if is_debugger:
                    relative = ('Debuggers', 'x64', *relative)
                elif '-x64-' in name or '-x86-' in name:
                    relative = ('Lib', '10.0.28000.0', *relative)
                target = output.joinpath(*relative).resolve()
                if not target.is_relative_to(output):
                    raise SystemExit(f'Package entry escapes SDK directory: {entry.filename}')
                target.parent.mkdir(parents=True, exist_ok=True)
                if target.exists():
                    if not unchanged(target, entry):
                        raise SystemExit(f'Existing SDK file differs; preserved: {target}')
                else:
                    with package.open(entry) as source, target.open('xb') as destination:
                        shutil.copyfileobj(source, destination)
                count += 1
    expected_files = ['Include/10.0.28000.0/um/Windows.h',
                      'Lib/10.0.28000.0/um/x64/kernel32.Lib',
                      'Lib/10.0.28000.0/ucrt/x64/ucrt.lib',
                      'bin/10.0.28000.0/x64/rc.exe', 'Debuggers/x64/dbghelp.dll']
    for path in expected_files:
        if not (output / path).is_file():
            raise SystemExit(f'SDK assembly is incomplete: {path}')
    manifest = {'sdkVersion': '10.0.28000.0', 'packageVersion': '10.0.28000.2270',
                'packageSha256': PACKAGES, 'fileCount': count,
                'files': {name: sha256(output / name) for name in expected_files}}
    (output / 'folio-sdk-provenance.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    print(f'Assembled private SDK: {count} files; required headers, libraries and resource compiler present')


if __name__ == '__main__':
    main()
