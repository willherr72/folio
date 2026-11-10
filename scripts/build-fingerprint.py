"""Capture/check product source together with the actual renderer dependency."""
from pathlib import Path
import hashlib
import json
import sys

root = Path(__file__).resolve().parent.parent
target = Path(sys.argv[2]).resolve() / 'build-source-fingerprint.json'
paths = [p for name in ['src', 'src-tauri/src'] for p in (root / name).rglob('*') if p.is_file()]
paths += [root / 'src-tauri/build.rs']
paths += [p for name in ['src-tauri/native', 'src-tauri/vendor/libjpeg-turbo-3.2.1']
          for p in (root / name).rglob('*') if p.is_file()]
# Local native bindings are part of the executable, not an interchangeable cache.
paths += [p for p in (root / 'src-tauri/vendor/pdfium-render-0.9.4').rglob('*')
          if p.is_file() and p.suffix in ('.rs', '.toml')]
# Parsing and writing patches are part of the executable's behavior too.
paths += [p for p in (root / 'src-tauri/vendor/lopdf-0.45.0').rglob('*')
          if p.is_file() and p.suffix in ('.rs', '.toml')]
paths += [root / name for name in ['package.json', 'package-lock.json', 'src-tauri/Cargo.toml',
          'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json',
          'src-tauri/resources/pdfium/pdfium.dll', 'src-tauri/resources/pdfium/provenance.json',
          'scripts/pdfium-source-lock.json', 'patches/pdfium/form-text-state-inheritance.patch',
          'patches/pdfium/fractional-horizontal-advances.patch', 'patches/pdfium/ordered-default-colors.patch',
          'patches/pdfium/embedded-form-font-preference.patch', 'patches/pdfium/fractional-form-borders.patch',
          'patches/pdfium/form-separator-coordinates.patch']]
files = {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)}
fingerprint = hashlib.sha256(json.dumps(files, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
if sys.argv[1] == 'write':
    with target.open('x', encoding='utf-8') as output:
        json.dump(dict(files=files, fingerprint=fingerprint), output, indent=2)
        output.write('\n')
elif sys.argv[1] == 'check':
    if json.loads(target.read_text(encoding='utf-8'))['files'] != files:
        raise SystemExit('Product source or renderer changed during build')
else:
    raise SystemExit('Expected write or check')
print(fingerprint)
