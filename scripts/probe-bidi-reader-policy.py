"""Bounded reader-policy probes on existing native-layout fixtures; no production widening."""
import argparse, importlib.util, json, hashlib
from pathlib import Path
from pypdf import PdfReader,PdfWriter
from pypdf.generic import DictionaryObject as D,NameObject as N,TextStringObject as T,BooleanObject as B
import fitz
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('reader_order',ROOT/'scripts/probe-reader-font-order.py');reader=importlib.util.module_from_spec(spec);spec.loader.exec_module(reader)
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=False)
 source=ROOT/'artifacts/shaped-text/semantic-rotations';inputs=json.loads((source/'inputs.json').read_text(encoding='utf-8'));pdf=PdfReader(source/'semantic.pdf');dll=reader.native_reader(ROOT/'src-tauri/resources/pdfium/pdfium.dll');dll.FPDF_InitLibrary();rows=[]
 try:
  for index,case in enumerate(inputs):
   if case['case'] not in ('mixed-bidi','arabic') or case['order'] not in ('visual-tj','logical-tj'):continue
   for policy in ('original','viewer-rtl','reversed-chars','tagged-rtl'):
    w=PdfWriter();page=w.add_page(pdf.pages[index]);content=page.get_contents().get_data()
    if policy=='viewer-rtl':w._root_object[N('/ViewerPreferences')]=D({N('/Direction'):N('/R2L')})
    if policy=='reversed-chars':content=content.replace(b'BT /S',b'/ReversedChars BMC\nBT /S')+b'\nEMC\n'
    if policy=='tagged-rtl':
     w._root_object[N('/ViewerPreferences')]=D({N('/Direction'):N('/R2L')});w._root_object[N('/Lang')]=T('ar');w._root_object[N('/MarkInfo')]=D({N('/Marked'):B(True)})
     # /Lang and viewer direction are hints; this is not claimed as a fully tagged PDF.
    page[N('/Contents')]=w._add_object(reader.stream(content));path=args.output/f"{case['case']}-{case['order']}-{case['rotation']}-{policy}.pdf";w.write(path)
    raw=reader.inspect(dll,path);mu=fitz.open(path);mt=mu[0].get_text();mu.close();pt=PdfReader(path).pages[0].extract_text();expected=case['text']
    rows.append(dict(case=case['case'],order=case['order'],rotation=case['rotation'],policy=policy,expected=expected,pdfium=raw,mupdf=mt,pypdf=pt,exact={k:v.rstrip('\r\n')==expected for k,v in [('pdfium',raw['text']),('mupdf',mt),('pypdf',pt)]},sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
 finally:dll.FPDF_DestroyLibrary()
 report={'sourcePdfSha256':hashlib.sha256((source/'semantic.pdf').read_bytes()).hexdigest(),'scope':'Existing native glyph positions unchanged; metadata/marked-content policy only. tagged-rtl is not a complete tagged-PDF implementation.','cases':rows};(args.output/'results.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
 for policy in ('original','viewer-rtl','reversed-chars','tagged-rtl'):
  group=[r for r in rows if r['policy']==policy];print(policy,{key:sum(r['exact'][key] for r in group) for key in ('pdfium','mupdf','pypdf')},'of',len(group))
if __name__=='__main__':main()
