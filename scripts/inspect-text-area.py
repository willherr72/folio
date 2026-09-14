"""Inspect the #16 development fixture without disguising soft-wrap copy failures.

Exact logical copy is compared byte-for-byte as Python Unicode strings. A separate
painted-line check ignores only CR/LF conventions and reader terminal line breaks;
it does not establish exact logical copying. Native/resaved raster and text must
remain stable for the evidence to be valid. No PDF is written or modified here.
Default success requires exact PDFium character-range text, MuPDF selection text,
and pypdf extraction, before and after save. Page extraction is retained separately.
--diagnostic is explicitly not acceptance. Interactive viewer approval is separate.
"""
import argparse
import hashlib
import json
import re
from copy import deepcopy
from pathlib import Path


def native_copy_text(path):
    # Reuse the raw-binding probe, not Folio's own clipboard implementation.
    import ctypes as c
    import importlib.util
    spec = importlib.util.spec_from_file_location('tagged_probe', Path(__file__).with_name('probe-tagged-selection.py'))
    probe = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(probe)
    dll = probe.separator.reader.native_reader(probe.ROOT / 'src-tauri/resources/pdfium/pdfium.dll')
    dll.FPDFText_GetText.argtypes = [c.c_void_p, c.c_int, c.c_int, c.POINTER(c.c_ushort)]
    dll.FPDFText_GetText.restype = c.c_int
    dll.FPDF_InitLibrary()
    try:
        return probe.pdfium_text(dll, path)
    finally:
        dll.FPDF_DestroyLibrary()


def reader_text_result(logical, painted_lines, actual):
    lines = actual.replace('\r\n', '\n').split('\n')
    if lines and lines[-1] == '':
        lines.pop()
    return {'actualText': actual, 'exactLogicalText': actual == logical,
            'logicalTextIgnoringOneTerminalEol': actual in [logical, logical + '\n', logical + '\r\n'],
            'paintedLineText': lines == painted_lines}


def validate_case_names(cases, expected):
    names = [case['name'] for case in cases]
    if expected < 1 or len(names) != expected or len(set(names)) != len(names):
        raise ValueError('Expected a nonempty, complete matrix with unique case names')


def validate_layout(case):
    layout, request = case['layout'], case['request']
    assert layout['request'] == request
    source = request['text']
    raw = source.encode('utf-8')
    cursor = 0
    reconstructed = []
    assert 1 <= len(layout['lines']) <= 256
    assert layout['lines'][-1]['breakKind'] == 'end'
    actual_painted = []
    for index, line in enumerate(layout['lines']):
        parts = []
        for key in ['source', 'delimiter']:
            span = line[key]
            start, end = span['utf8Start'], span['utf8End']
            assert start == cursor and start <= end <= len(raw), (case['name'], key, span)
            prefix = raw[:start].decode('utf-8')
            part = raw[start:end].decode('utf-8')
            assert span['utf16Start'] == len(prefix.encode('utf-16-le')) // 2
            assert span['utf16End'] == len((prefix + part).encode('utf-16-le')) // 2
            cursor = end
            reconstructed.append(part)
            parts.append(part)
        kind = line['breakKind']
        if kind == 'soft':
            assert parts[1] and set(parts[1]) == {' '}
        elif kind == 'hard':
            ending = '\r\n' if parts[1].endswith('\r\n') else '\n'
            assert parts[1].endswith(ending) and not parts[1][:-len(ending)].strip(' ')
        else:
            assert kind == 'end' and not parts[1].strip(' ') and index == len(layout['lines']) - 1
        assert bool(parts[0]) == (line['shaped'] is not None)
        actual_painted.append(parts[0])
        if line['shaped'] is not None:
            assert line['shaped']['text'] == parts[0]
            assert line['shaped']['fontId'] == layout['fontId']
    assert cursor == len(raw) and ''.join(reconstructed) == source
    assert case['paintedLines'] == actual_painted
    assert layout['canExport'] == (not any(layout['overflow'].values()))
    return True


def geometry_without_resource_labels(raw):
    # MuPDF labels unnamed Type3 fonts with their PDF object reference. Page
    # import legitimately renumbers those resources; no geometry is normalized.
    result = deepcopy(raw)
    for block in result.get('blocks', []):
        for line in block.get('lines', []):
            for span in line.get('spans', []):
                if re.fullmatch(r'Type3 \(\d+ \d+ R\)', span.get('font', '')):
                    span['font'] = 'Type3 (resource reference)'
    return result


def inspect(input_path, expected):
    import fitz
    import pypdf
    manifest = json.loads((input_path / 'results.json').read_text(encoding='utf-8'))
    validate_case_names(manifest['cases'], expected)
    assert manifest['sourceUnchanged']
    results = []
    for case in manifest['cases']:
        row = {'name': case['name'], 'exactSourceRanges': validate_layout(case),
               'logicalText': case['request']['text'], 'overflow': case['layout']['overflow']}
        if not case['layout']['canExport']:
            assert case['exportRefused'] and case['pdf'] is None and case['resavedPdf'] is None
            row.update(exportRefused=True, roundtripStable=True)
            results.append(row)
            continue
        assert not case['exportRefused']
        painted = [text for text in case['paintedLines'] if text]
        row['paintedLines'] = painted
        row['readers'] = {}
        paths = [input_path / case[key] for key in ['pdf', 'resavedPdf']]
        texts = []
        copies = []
        windows_copies = []
        rasters = []
        geometry = []
        for path in paths:
            with fitz.open(path) as document:
                assert len(document) == 1
                page = document[0]
                mu_text = page.get_text(sort=False)
                textpage = page.get_textpage()
                start = fitz.mupdf.FzPoint(0, 0)
                end = fitz.mupdf.FzPoint(page.cropbox.width, page.cropbox.height)
                copies.append({'mupdf': fitz.mupdf.fz_copy_selection(textpage.this, start, end, 0),
                               'pdfium': native_copy_text(path)})
                windows_copies.append(fitz.mupdf.fz_copy_selection(textpage.this, start, end, 1))
                row['mupdfSelectionPoints'] = [[start.x, start.y], [end.x, end.y]]
                geometry.append(page.get_text('rawdict'))
                pixmap = page.get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False)
                rasters.append((pixmap.width, pixmap.height, hashlib.sha256(pixmap.samples).hexdigest()))
            reader = pypdf.PdfReader(path)
            assert len(reader.pages) == 1
            texts.append({'mupdf': mu_text, 'pypdf': reader.pages[0].extract_text()})
        texts[0]['pdfium'] = case['pdfiumText']
        texts[1]['pdfium'] = case['pdfiumResavedText']
        assert copies[0]['pdfium'] == texts[0]['pdfium'] and copies[1]['pdfium'] == texts[1]['pdfium'], 'Fresh PDFium text differs from native fixture evidence'
        for reader in ['pdfium', 'mupdf', 'pypdf']:
            row['readers'][reader] = reader_text_result(row['logicalText'], painted, texts[0][reader])
            row['readers'][reader]['resavedText'] = texts[1][reader]
            if reader in copies[0]:
                row['readers'][reader].update(copyText=copies[0][reader], resavedCopyText=copies[1][reader],
                                             exactCopyText=copies[0][reader] == row['logicalText'],
                                             copyMethod='FPDFText_GetText full character range' if reader == 'pdfium' else 'fz_copy_selection page corners, crlf=0')
        row['readers']['mupdf'].update(selectionCrlf=windows_copies[0], resavedSelectionCrlf=windows_copies[1])
        row['mupdfRawDictionaryEqual'] = geometry[0] == geometry[1]
        row['mupdfGeometryEqualExceptResourceLabels'] = geometry_without_resource_labels(geometry[0]) == geometry_without_resource_labels(geometry[1])
        row['roundtripStable'] = (texts[0] == texts[1] and copies[0] == copies[1] and windows_copies[0] == windows_copies[1] and row['mupdfGeometryEqualExceptResourceLabels']
                                  and rasters[0] == rasters[1]
                                  and case['pdfiumGeometry'] == case['pdfiumResavedGeometry']
                                  and all(raster['unchanged'] for raster in case['rasters']))
        row['mupdfRasterSha256'] = rasters[0][2]
        row['pdfSha256'] = hashlib.sha256(paths[0].read_bytes()).hexdigest()
        row['allReadersExactLogicalText'] = all(r['exactLogicalText'] for r in row['readers'].values())
        row['exactSelectionAndExtraction'] = all(value == row['logicalText'] for copy in copies for value in copy.values()) and all(text['pypdf'] == row['logicalText'] for text in texts)
        row['allReadersLogicalIgnoringOneTerminalEol'] = all(r['logicalTextIgnoringOneTerminalEol'] for r in row['readers'].values())
        row['allReadersPaintedLineText'] = all(r['paintedLineText'] for r in row['readers'].values())
        results.append(row)
    exported = [row for row in results if 'readers' in row]
    assert exported, 'At least one exported case is required'
    valid = all(row['roundtripStable'] and row['exactSourceRanges'] for row in results)
    return {'description': 'Development evidence; painted-line agreement is not exact logical copy.',
            'copyEvidenceVersion': 2,
            'mupdf': fitz.VersionBind, 'pypdf': pypdf.__version__,
            'evidenceValid': valid, 'cases': len(results), 'exports': len(exported),
            'exactLogicalCopyCases': sum(row['allReadersExactLogicalText'] for row in exported),
            'exactSelectionAndExtractionCases': sum(row['exactSelectionAndExtraction'] for row in exported),
            'logicalIgnoringOneTerminalEolCases': sum(row['allReadersLogicalIgnoringOneTerminalEol'] for row in exported),
            'paintedLineCases': sum(row['allReadersPaintedLineText'] for row in exported),
            'results': results}


def exact_copy_gate(report):
    """Necessary release gate only; this does not establish all area acceptance."""
    exported = [row for row in report.get('results', []) if 'readers' in row]
    if not report.get('evidenceValid') or not exported or len(exported) != report.get('exports'):
        return False
    return all(row.get('roundtripStable') and row.get('exactSourceRanges')
               and isinstance(row.get('logicalText'), str)
               and all(row['readers'].get(reader, {}).get(key) == row['logicalText']
                       for reader in ['pdfium', 'mupdf'] for key in ['copyText', 'resavedCopyText'])
               and all(row['readers'].get('pypdf', {}).get(key) == row['logicalText']
                       for key in ['actualText', 'resavedText']) for row in exported)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, default=Path('artifacts/text-area-native'))
    parser.add_argument('--expected-cases', type=int, required=True)
    parser.add_argument('--diagnostic', action='store_true', help='Report failed copying without treating it as release acceptance; still require valid fixture/roundtrip evidence.')
    parser.add_argument('--output', type=Path, default=Path('artifacts/text-area-reader-results.json'))
    args = parser.parse_args()
    report = inspect(args.input, args.expected_cases)
    report['exactCopyGatePassed'] = exact_copy_gate(report)
    report['diagnosticOnly'] = args.diagnostic
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({key: value for key, value in report.items() if key != 'results'}))
    if not report['evidenceValid'] or (not args.diagnostic and not report['exactCopyGatePassed']):
        raise SystemExit(1)

if __name__ == '__main__':
    main()
