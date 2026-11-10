"""Compare Word exports using Folio's existing unnormalized reader operations.

Diagnostic completion is not a passing exact-copy gate or desktop acceptance.
"""
import argparse
import ctypes as c
import hashlib
import importlib.util
import json
from pathlib import Path

import fitz
import pypdf

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('tagged', ROOT / 'scripts/probe-tagged-selection.py')
tagged = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tagged)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inspect(dll, path, expected):
    pdf = pypdf.PdfReader(path)
    assert len(pdf.pages) == 1, 'Only one-page controls are supported'
    mu = tagged.mupdf_selection(path)
    native = tagged.separator.reader.inspect(dll, path)
    texts = {
        'pdfium': tagged.pdfium_text(dll, path),
        'mupdfSelectionLf': mu['selectionLf'],
        'mupdfSelectionCrlf': mu['selectionCrlf'],
        'mupdfPageText': mu['pageText'],
        'pypdf': pdf.pages[0].extract_text(),
    }
    with fitz.open(path) as doc:
        lines = [''.join(span['text'] for span in line['spans'])
                 for block in doc[0].get_text('dict')['blocks']
                 for line in block.get('lines', [])]
    content = pdf.pages[0].get_contents().get_data()
    path.with_suffix('.content.txt').write_bytes(content)
    return {
        'pdf': path.name, 'sha256': sha(path),
        'texts': texts, 'exact': {key: value == expected for key, value in texts.items()},
        'mupdfLines': lines, 'mupdfCharacters': mu['characters'],
        'pdfiumCharacters': native['characters'], 'raster': mu['raster'],
        'selectionPoints': [mu['selectionStart'], mu['selectionEnd']],
        'hasStructureTree': '/StructTreeRoot' in pdf.trailer['/Root'],
        'contentSha256': hashlib.sha256(content).hexdigest(),
        'producer': str((pdf.metadata or {}).get('/Producer', '')),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    manifest_path = args.input / 'manifest.json'
    manifest = json.loads(manifest_path.read_text(encoding='utf-8-sig'))
    assert len(manifest['cases']) == 12
    assert len({row['pdf'] for row in manifest['cases']}) == 12
    expected_names = {'single', 'spaces', 'soft', 'soft-spaces', 'hard', 'hard-equivalent'}
    assert {(row['name'], row['tagged']) for row in manifest['cases']} == {
        (name, tags) for name in expected_names for tags in [True, False]}
    dllpath = ROOT / 'src-tauri/resources/pdfium/pdfium.dll'
    dll = tagged.separator.reader.native_reader(dllpath)
    dll.FPDFText_GetText.argtypes = [c.c_void_p, c.c_int, c.c_int, c.POINTER(c.c_ushort)]
    dll.FPDFText_GetText.restype = c.c_int
    dll.FPDF_InitLibrary()
    rows = []
    try:
        for source in manifest['cases']:
            path = args.input / source['pdf']
            assert sha(path) == source['pdfSha256'], 'Source PDF hash mismatch'
            assert sha(args.input / source['docx']) == source['docxSha256'], 'Source DOCX hash mismatch'
            original = inspect(dll, path, source['expected'])
            assert original['hasStructureTree'] == source['tagged']
            assert len(original['mupdfLines']) == source['wordLines'], 'Export row count differs'
            resaved_path = args.output / path.with_stem(path.stem + '-resaved').name
            with fitz.open(path) as doc:
                doc.save(resaved_path, garbage=4, deflate=True)
            resaved = inspect(dll, resaved_path, source['expected'])
            stable = all(original[key] == resaved[key] for key in [
                'texts', 'mupdfCharacters', 'pdfiumCharacters', 'raster', 'selectionPoints'])
            rows.append({
                'case': source['name'], 'tagged': source['tagged'], 'expected': source['expected'],
                'original': original, 'resaved': resaved, 'roundtripStable': stable,
                'exactCopyGate': all(original['exact'][key] and resaved['exact'][key]
                                     for key in ['pdfium', 'mupdfSelectionLf', 'pypdf']),
            })
    finally:
        dll.FPDF_DestroyLibrary()
    assert all(row['roundtripStable'] for row in rows), 'Roundtrip changed measured results'
    comparisons = []
    for tags in [True, False]:
        soft = next(row['original'] for row in rows if row['case'] == 'soft' and row['tagged'] == tags)
        hard = next(row['original'] for row in rows if row['case'] == 'hard-equivalent' and row['tagged'] == tags)
        comparisons.append({'tagged': tags, 'pair': ['soft', 'hard-equivalent'],
                            'sameRaster': soft['raster'] == hard['raster'],
                            'sameCopy': soft['texts'] == hard['texts'],
                            'sameTextByMethod': {key: soft['texts'][key] == hard['texts'][key]
                                                 for key in soft['texts']},
                            'sameCharacters': soft['mupdfCharacters'] == hard['mupdfCharacters'],
                            'sameContentStream': soft['contentSha256'] == hard['contentSha256']})
    tag_comparisons = []
    for name in sorted(expected_names):
        a, b = [row['original'] for row in rows if row['case'] == name]
        tag_comparisons.append({'case': name, 'sameEvidence': all(a[key] == b[key] for key in [
            'texts', 'mupdfCharacters', 'pdfiumCharacters', 'raster'])})
    report = {'description': __doc__, 'releaseAcceptance': False,
              'roundtripMethod': 'MuPDF save, garbage=4, deflate=True; not Folio native export',
              'manifest': manifest, 'manifestSha256': sha(manifest_path),
              'mupdf': fitz.VersionBind, 'pypdf': pypdf.__version__,
              'pdfiumSha256': sha(dllpath), 'cases': rows, 'softVersusHard': comparisons,
              'taggedVersusUntagged': tag_comparisons}
    (args.output / 'results.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    for row in rows:
        print(row['case'], 'tagged' if row['tagged'] else 'untagged',
              json.dumps(row['original']['texts']), 'exactCopyGate', row['exactCopyGate'])
    print('Soft versus hard:', json.dumps(comparisons))


if __name__ == '__main__':
    main()
