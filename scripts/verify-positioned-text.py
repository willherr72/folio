"""Compare native issue15 exports with separately authored reference PDFs.
Generate fixtures with FOLIO_POSITIONED_ARTIFACTS and the positioned_single_runs Rust test.
"""
import argparse, hashlib, json
from pathlib import Path
import fitz
import pypdf

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--input',type=Path,default=Path('artifacts/positioned-native'))
parser.add_argument('--output',type=Path,default=Path('docs/issue15-reader-verification.json'))
args=parser.parse_args()
results=[]
for output in sorted(args.input.glob('*-exported.pdf')):
    reference=output.with_name(output.name.replace('-exported.pdf','-reference.pdf'))
    with fitz.open(output) as actual,fitz.open(reference) as expected:
        text=actual[0].get_text(sort=False)
        expected_text=expected[0].get_text(sort=False)
        a=actual[0].get_pixmap(matrix=fitz.Matrix(2,2),alpha=False)
        b=expected[0].get_pixmap(matrix=fitz.Matrix(2,2),alpha=False)
        raster=a.width==b.width and a.height==b.height and a.samples==b.samples
    extracted=pypdf.PdfReader(output).pages[0].extract_text()
    reference_extracted=pypdf.PdfReader(reference).pages[0].extract_text()
    record={'case':output.stem.removesuffix('-exported'),'sha256':hashlib.sha256(output.read_bytes()).hexdigest(),'mupdfExactText':text==expected_text,'mupdfExactRaster':raster,'pypdfExactText':extracted==reference_extracted,'mupdfText':text,'pypdfText':extracted}
    results.append(record)
assert len(results)==144,f'Expected144 native cases; got{len(results)}'
report={'mupdf':fitz.VersionBind,'pypdf':pypdf.__version__,'cases':len(results),'passed':all(r['mupdfExactText'] and r['mupdfExactRaster'] and r['pypdfExactText'] for r in results),'results':results}
args.output.parent.mkdir(parents=True,exist_ok=True)
args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in report.items() if k!='results'}))
if not report['passed']:
    print(json.dumps([r for r in results if not all(r[k] for k in ['mupdfExactText','mupdfExactRaster','pypdfExactText'])],ensure_ascii=False,indent=2))
    raise SystemExit(1)
