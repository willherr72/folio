"""Record an operator's external-viewer copy against a hashed synthetic fixture.

Run immediately after copying the complete specimen in the named viewer. This
does not control the viewer or prove the clipboard's provenance. A matching string
alone is not release acceptance; interactive selection geometry and roundtrips
remain separate requirements. Reports may contain clipboard data: keep them local.
"""
import argparse
import hashlib
import json
from datetime import datetime, timezone
from itertools import zip_longest
from pathlib import Path


def compare(expected, actual):
    difference = None
    for index, (a, b) in enumerate(zip_longest(expected, actual)):
        if a != b:
            difference = {'scalarIndex': index,
                          'expected': f'U+{ord(a):04X}' if a is not None else 'END',
                          'actual': f'U+{ord(b):04X}' if b is not None else 'END'}
            break
    return {'expected': expected, 'actual': actual, 'exact': expected == actual,
            'expectedScalars': len(expected), 'actualScalars': len(actual),
            'firstDifference': difference}


def fixture(manifest, name):
    if Path(name).name != name:
        raise ValueError('Choose a fixture filename, not a path')
    rows = json.loads(manifest.read_text(encoding='utf-8'))['cases']
    matches = [row for row in rows if row['pdf'] == name]
    if len(matches) != 1:
        raise ValueError('Fixture must have exactly one manifest entry')
    row = matches[0]
    path = (manifest.parent / name).resolve(strict=True)
    if path.parent != manifest.parent.resolve():
        raise ValueError('Fixture must stay in the manifest directory')
    if hashlib.sha256(path.read_bytes()).hexdigest() != row['sha256']:
        raise ValueError('PDF changed since the evidence was generated')
    if not isinstance(row['expected'], str) or not row['expected']:
        raise ValueError('Expected text must be nonempty')
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--pdf', required=True)
    parser.add_argument('--viewer', required=True, help='Viewer name and exact installed version')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--from-file', type=Path, help='Offline UTF-8 comparison; never recorded as clipboard evidence')
    args = parser.parse_args()
    if args.output.exists():
        raise ValueError('Refusing to overwrite an existing report')
    row = fixture(args.manifest, args.pdf)
    if args.from_file:
        # read_text() would translate CRLF; preserve every input byte here.
        actual = args.from_file.read_bytes().decode('utf-8')
        source = 'offline-file'
    else:
        import win32clipboard
        win32clipboard.OpenClipboard()
        try:
            if not win32clipboard.IsClipboardFormatAvailable(win32clipboard.CF_UNICODETEXT):
                raise ValueError('Clipboard has no Unicode text; copy the specimen first')
            actual = win32clipboard.GetClipboardData(win32clipboard.CF_UNICODETEXT)
        finally:
            win32clipboard.CloseClipboard()
        source = 'operator-reported-clipboard'
    result = compare(row['expected'], actual)
    result.update(source=source, viewer=args.viewer, pdf=row['pdf'], pdfSha256=row['sha256'],
                  recordedAt=datetime.now(timezone.utc).isoformat(), releaseAcceptance=False)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open('x', encoding='utf-8', newline='\n') as output:
        json.dump(result, output, ensure_ascii=False, indent=2)
        output.write('\n')
    print(json.dumps({key: result[key] for key in ['source', 'exact', 'expectedScalars', 'actualScalars', 'firstDifference']}))
    if not result['exact']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
