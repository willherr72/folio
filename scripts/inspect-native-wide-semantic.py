"""Inspect actual native wide-CID fixtures and exported PDFs in three readers.
Run after shaped-text-probe --semantic-wide. Does not generate PDF content.
"""
import argparse,ctypes as c,hashlib,importlib.util,io,json,subprocess,sys
from pathlib import Path
import fitz
from fontTools.ttLib import TTFont
from pypdf import PdfReader
from pypdf.generic import ContentStream
ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--input',type=Path,default=ROOT/'artifacts/shaped-text/semantic-wide-native')
parser.add_argument('--output',type=Path,default=ROOT/'artifacts/issue14-wide/native-inspection')
parser.add_argument('--summary',type=Path,default=ROOT/'docs/issue14-native-wide-verification.json')
options=parser.parse_args(); options.output.mkdir(parents=True,exist_ok=True)
spec=importlib.util.spec_from_file_location('native_inspect',ROOT/'scripts/inspect-semantic-native.py'); ins=importlib.util.module_from_spec(spec); spec.loader.exec_module(ins)
DLL=ROOT/'src-tauri/resources/pdfium/pdfium.dll'; dll=c.CDLL(str(DLL))
for name,args,result in [('FPDF_InitLibrary',[],None),('FPDF_DestroyLibrary',[],None),('FPDF_LoadDocument',[c.c_char_p,c.c_char_p],c.c_void_p),('FPDF_CloseDocument',[c.c_void_p],None),('FPDF_LoadPage',[c.c_void_p,c.c_int],c.c_void_p),('FPDF_ClosePage',[c.c_void_p],None),('FPDFText_LoadPage',[c.c_void_p],c.c_void_p),('FPDFText_ClosePage',[c.c_void_p],None),('FPDFText_CountChars',[c.c_void_p],c.c_int),('FPDFText_GetUnicode',[c.c_void_p,c.c_int],c.c_uint),('FPDFText_GetCharBox',[c.c_void_p,c.c_int]+[c.POINTER(c.c_double)]*4,c.c_int),('FPDFText_GetText',[c.c_void_p,c.c_int,c.c_int,c.POINTER(c.c_ushort)],c.c_int)]:
 f=getattr(dll,name); f.argtypes=args; f.restype=result

def pdfium(path):
 doc=page=text=None
 try:
  doc=dll.FPDF_LoadDocument(str(path).encode('utf-8'),None); assert doc
  page=dll.FPDF_LoadPage(doc,0); assert page
  text=dll.FPDFText_LoadPage(page); assert text
  chars=[]
  for i in range(dll.FPDFText_CountChars(text)):
   v=[c.c_double() for _ in range(4)]; assert dll.FPDFText_GetCharBox(text,i,*[c.byref(x) for x in v])
   chars.append({'text':chr(dll.FPDFText_GetUnicode(text,i)),'box':[x.value for x in v]})
  buf=(c.c_ushort*(len(chars)*2+1))(); written=dll.FPDFText_GetText(text,0,len(chars),buf)
  raw=bytes(buf)[:max(0,written-1)*2].decode('utf-16-le')
  scalars=[]; i=0
  while i<len(chars):
   first=ord(chars[i]['text'])
   if 0xD800<=first<=0xDBFF:
    assert i+1<len(chars) and 0xDC00<=ord(chars[i+1]['text'])<=0xDFFF
    assert chars[i]['box']==chars[i+1]['box'], repr(chars[i:i+2])
    scalar=0x10000+((first-0xD800)<<10)+(ord(chars[i+1]['text'])-0xDC00)
    scalars.append({'text':chr(scalar),'box':chars[i]['box'],'raw_utf16_units':[first,ord(chars[i+1]['text'])]}); i+=2
   else:
    assert not 0xDC00<=first<=0xDFFF
    scalars.append(chars[i]); i+=1
  return scalars,raw
 finally:
  if text:dll.FPDFText_ClosePage(text)
  if page:dll.FPDF_ClosePage(page)
  if doc:dll.FPDF_CloseDocument(doc)

def inspect(path,entry,layout,variant):
 reader=PdfReader(path); assert len(reader.pages)==1
 page=reader.pages[0]; fonts=[f.get_object() for f in page['/Resources']['/Font'].values()]
 sem=[f for f in fonts if '/ToUnicode' in f]; orig=[f for f in fonts if '/ToUnicode' not in f]
 assert len(sem)==len(orig)==1 and all(f['/Subtype']=='/Type0' for f in fonts)
 original=orig[0]['/DescendantFonts'][0].get_object()['/FontDescriptor']['/FontFile2'].get_data()
 assert hashlib.sha256(original).hexdigest()==layout['fontId']==entry['fontSha256']
 sf=sem[0]; cid=sf['/DescendantFonts'][0].get_object(); data=cid['/FontDescriptor']['/FontFile2'].get_data(); tt=TTFont(io.BytesIO(data),checkChecksums=2)
 assert sf['/Encoding']=='/Identity-H' and cid['/Subtype']=='/CIDFontType2' and tt['head'].unitsPerEm==1000
 order=tt.getGlyphOrder(); assert cid['/CIDToGIDMap'].get_data()==b''.join(i.to_bytes(2,'big') for i in range(len(order)))
 assert len(cid['/W'][1])==len(order)-1>255
 ops=ContentStream(page.get_contents(),reader).operations; operators=[op for _,op in ops]
 assert all(operators.count(op)==1 for op in (b'BT',b'ET',b'TJ',b'Tf',b'Tm',b'Tr'))
 assert next(values for values,op in ops if op==b'Tr')==[3]
 assert float(next(values for values,op in ops if op==b'Tf')[1])==layout['fontSize']/16
 expected=ins.expected_clusters(layout); chars,raw=pdfium(path)
 with fitz.open(path) as doc:
  page_mu=doc[0]; mchars=[ch for block in page_mu.get_text('rawdict')['blocks'] for line in block.get('lines',[]) for span in line['spans'] for ch in span['chars']]
  mutext=''.join(ch['c'] for ch in mchars); raster=hashlib.sha256(page_mu.get_pixmap().samples).hexdigest()
 raw_readers={'pdfium':raw,'mupdf':mutext,'pypdf':page.extract_text()}
 exact=all(t.rstrip('\r\n')==layout['text'] for t in raw_readers.values())
 assert exact and ''.join(ch['text'] for ch in chars)==layout['text'], repr({'file':str(path),'source_tail':layout['text'][-20:],'raw_tails':{k:v[-20:] for k,v in raw_readers.items()},'char_tail':''.join(ch['text'] for ch in chars)[-20:]})
 assert int(page.get('/Rotate',0))==entry['rotation']
 return {'case':entry['case']+'-'+variant,'rotation':entry['rotation'],'pdf':str(path.resolve()),'source':layout['text'],'output_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'expected':expected,'characters':{'pdfium':chars},'raw':raw_readers,'exact_all_readers':exact,'original_font_sha256':hashlib.sha256(original).hexdigest(),'semantic_font_sha256':hashlib.sha256(data).hexdigest(),'definitions':len(order)-1,'mupdf_raster_sha256':raster,'mupdf_characters':mchars,'visible_prefix_sha256':hashlib.sha256(page.get_contents().get_data().split(b'BT',1)[0]).hexdigest()}

evidence=json.loads((options.input/'results.json').read_text(encoding='utf-8')); rows=[]; pairs=[]
expected_cases={(name,rotation) for name in ('bank-boundary-255','bank-boundary-256','bank-boundary-511','wide-ligatures','wide-marks','wide-supplementary') for rotation in (0,90,180,270)}
assert len(evidence['cases'])==24 and {(r['case'],r['rotation']) for r in evidence['cases']}==expected_cases, 'Native wide matrix incomplete'
dll.FPDF_InitLibrary()
try:
 for entry in evidence['cases']:
  source=options.input/entry['pdf']; layout=json.loads(source.with_suffix('.layout.json').read_text(encoding='utf-8'))
  a=inspect(source,entry,layout,'original'); b=inspect(options.input/entry['resavedPdf'],entry,layout,'resaved'); rows.extend([a,b])
  checks={key:a[key]==b[key] for key in ['raw','characters','original_font_sha256','semantic_font_sha256','mupdf_raster_sha256','mupdf_characters','visible_prefix_sha256']}
  assert all(checks.values()) and entry['renderUnchanged']
  pairs.append({'case':entry['case'],'rotation':entry['rotation'],'unchanged':checks,'native_pdfium_render_unchanged':entry['renderUnchanged']})
  print(entry['case'],entry['rotation'],'three-reader original/resaved exact',flush=True)
finally:dll.FPDF_DestroyLibrary()
report={'pdfium_sha256':hashlib.sha256(DLL.read_bytes()).hexdigest(),'cases':rows,'pairs':pairs}
(options.output/'results.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
subprocess.run([sys.executable,'-B','-X','utf8',str(ROOT/'scripts/verify-wide-semantic-geometry.py'),'--input',str(options.output),'--output',str(options.output/'geometry-model.json')],check=True)

model=json.loads((options.output/'geometry-model.json').read_text(encoding='utf-8'))
summary={'scope':'Actual native wide CID fixtures, independently read before and after native export; PDFium source-model top padding is explicit. No physical reader or UI acceptance claim.','pdfium_sha256':report['pdfium_sha256'],'reader_pdf_count':len(rows),'pair_count':len(pairs),'pairs':pairs,'source_model':{k:v for k,v in model.items() if k!='cases'},'cases':[]}
for row,geometry in zip(rows,model['cases']):
 summary['cases'].append({k:row[k] for k in ['case','rotation','source','output_sha256','raw','exact_all_readers','original_font_sha256','semantic_font_sha256','definitions','mupdf_raster_sha256','visible_prefix_sha256']} | {k:v for k,v in geometry.items() if k not in ['predicted','observed']})
options.summary.write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
print(f"Native wide verification passed: {len(rows)} PDFs / {len(pairs)} original-export pairs")
