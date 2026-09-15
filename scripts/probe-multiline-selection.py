"""Experimental real-baseline control; no production writer or release change."""
import argparse
import ctypes as c
import hashlib
import importlib.util
import json
from pathlib import Path

import fitz
import pypdf

spec = importlib.util.spec_from_file_location('tagged', Path(__file__).with_name('probe-tagged-selection.py'))
tagged = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tagged)
separator = tagged.separator


def inspect(dll, path, logical, cells):
    mu = tagged.mupdf_selection(path)
    raw = separator.reader.inspect(dll, path)
    painted_cells = [cell for cell in cells if cell['text'] != '\n']
    native_chars = [char for char in raw['characters'] if char['text'] not in ['\r', '\n']]
    texts = {'pdfium': tagged.pdfium_text(dll, path), 'mupdfSelectionLf': mu['selectionLf'],
             'mupdfSelectionCrlf': mu['selectionCrlf'], 'mupdfPageText': mu['pageText'],
             'pypdf': pypdf.PdfReader(path).pages[0].extract_text()}
    chars = mu['characters']
    origins = len(chars) == len(painted_cells) and all(
        char['c'] == cell['text'] and
        max(abs(a-b) for a,b in zip(char['origin'], [cell['x'], 180-cell['y']])) < .02
        for char,cell in zip(chars,painted_cells))
    return {'pdf': path.name, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
            'texts': texts, 'exact': {key: value == logical for key,value in texts.items()},
            'mupdfOriginsExact': origins, 'pdfiumPaintedCellBoxesExact': separator.exact_boxes(native_chars, painted_cells),
            'mupdfCharacters': chars, 'pdfiumCharacters': raw['characters'], 'raster': mu['raster'],
            'selectionPoints': [mu['selectionStart'], mu['selectionEnd']]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    dllpath = separator.ROOT / 'src-tauri/resources/pdfium/pdfium.dll'
    dll = separator.reader.native_reader(dllpath)
    dll.FPDFText_GetText.argtypes = [c.c_void_p,c.c_int,c.c_int,c.POINTER(c.c_ushort)]
    dll.FPDFText_GetText.restype = c.c_int
    dll.FPDF_InitLibrary()
    rows = []
    try:
        for name,logical,wrap in tagged.CASES:
            for rotation in [0,90,180,270]:
                reference = None
                for strategy in ['scalar-cells','positioned-cells']:
                    path = args.output / f'{name}-{rotation}-{strategy}.pdf'
                    cells = separator.write_case(path,logical,wrap,strategy,rotation)
                    original = inspect(dll,path,logical,cells)
                    resaved_path = path.with_stem(path.stem+'-resaved')
                    with fitz.open(path) as document:
                        document.save(resaved_path,garbage=4,deflate=True)
                    resaved = inspect(dll,resaved_path,logical,cells)
                    if reference is None:
                        reference = original['raster']
                    stable = all(original[key] == resaved[key] for key in [
                        'texts','mupdfCharacters','pdfiumCharacters','raster','selectionPoints'])
                    rows.append({'case':name,'rotation':rotation,'strategy':strategy,'expected':logical,
                                 'original':original,'resaved':resaved,'roundtripStable':stable,
                                 'unchangedInk':original['raster']==reference,
                                 'exactCopyGate':all(original['exact'][key] and resaved['exact'][key]
                                                    for key in ['pdfium','mupdfSelectionLf','pypdf'])})
    finally:
        dll.FPDF_DestroyLibrary()
    assert len(rows)==40 and all(row['unchangedInk'] and row['roundtripStable'] for row in rows)
    report={'description':__doc__,'releaseAcceptance':False,'roundtripMethod':'MuPDF save, garbage=4, deflate=True; not Folio native export',
            'mupdf':fitz.VersionBind,'pypdf':pypdf.__version__,
            'pdfiumSha256':hashlib.sha256(dllpath.read_bytes()).hexdigest(),'cases':rows}
    (args.output/'results.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    for strategy in ['scalar-cells','positioned-cells']:
        group=[row for row in rows if row['strategy']==strategy]
        print(strategy,'complete copy passes',sum(row['exactCopyGate'] for row in group),
              'MuPDF origins',sum(row['original']['mupdfOriginsExact'] for row in group),'of',len(group))
    for row in rows:
        if row['rotation']==0 and row['strategy']=='positioned-cells':
            print(row['case'],json.dumps(row['original']['texts']))


if __name__=='__main__':
    main()
