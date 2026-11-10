"""Compare ordered DA color instructions with their canonical native appearance."""
import argparse,ctypes as c,hashlib,json,runpy
from pathlib import Path
from PIL import Image,ImageChops
from pypdf import PdfWriter
from pypdf.generic import DictionaryObject as D,NameObject as N,ArrayObject as A,NumberObject as I,BooleanObject as B,TextStringObject as T
helpers=runpy.run_path(str(Path(__file__).with_name('probe-pdfium-form-inheritance.py')))
CASES=[('gray-rgb','0 g 1 0 0 rg','1 0 0 rg'),('rgb-rgb','0 1 0 rg 1 0 0 rg','1 0 0 rg'),('gray-gray','0 g .5 g','.5 g'),('gray-cmyk','0 g 0 1 1 0 k','0 1 1 0 k'),('cmyk-cmyk','1 0 0 0 k 0 1 1 0 k','0 1 1 0 k'),('rgb-gray','1 0 0 rg .5 g','.5 g'),('cmyk-rgb','1 1 0 0 k 1 0 0 rg','1 0 0 rg'),('comments','0 g % misleading 0 1 0 rg\n1 0 0 rg','1 0 0 rg'),('rgb-control','0.2 0.4 0.6 rg','0.2 0.4 0.6 rg'),('gray-control','.5 g','.5 g')]
CASES += [('name-token','/g 1 0 0 rg','1 0 0 rg'),('literal-token','(0 g) 1 0 0 rg','1 0 0 rg'),('hex-token','<302067> 1 0 0 rg','1 0 0 rg'),('stroke-only','0 0 1 rg 0 G','0 0 1 rg'),('numeric-boundary','0 0 1 rg 1 0 0 Tc rg','0 0 1 rg'),('clipped-components','2 -1 .5 rg','1 0 .5 rg')]
CASES += [('escaped-close',r'0 g (\) 1 0 0 rg)','0 g'),('escaped-open',r'0 g (\() 1 0 0 rg','1 0 0 rg')]
CASES += [
 ('string-gray',r'1 0 0 rg (\) 0 g)','1 0 0 rg'),
 ('string-cmyk',r'0 g (\) 0 1 1 0 k)','0 g'),
 ('escaped-backslash',r'0 g (\\) 1 0 0 rg','1 0 0 rg'),
 ('nested-string',r'0 g (outer (inner \) 1 0 0 rg) end)','0 g'),
 ('octal-string',r'0 g (\050 1 0 0 rg \051)','0 g'),
 ('continued-string','0 g (\\\r\n1 0 0 rg)','0 g'),
 ('unterminated-string',r'0 g (\) 1 0 0 rg','0 g'),
 ('trailing-name','0 g /rg','0 g'),
 ('trailing-comment','0 g % 1 0 0 rg','0 g'),
 ('signed-fractions','+.2 +.4 +.6 rg','.2 .4 .6 rg'),
 ('trailing-decimal','1. 0. 0. rg','1 0 0 rg'),
 ('incomplete-operands','0 g 1 0 rg','0 g'),
 ('invalid-number','0 g 1e0 0 0 rg','0 g'),
 ('overflow-number','0 g 999999999999999999999999999999999999999 g','0 g'),
 ('cmyk-clipping','-1 2 2 -1 k','0 1 1 0 k'),
]
def fixture(path,color):
 w=PdfWriter();p=w.add_blank_page(320,240);font=w._add_object(D({N('/Type'):N('/Font'),N('/Subtype'):N('/Type1'),N('/BaseFont'):N('/Helvetica')}));field=w._add_object(D({N('/Type'):N('/Annot'),N('/Subtype'):N('/Widget'),N('/FT'):N('/Tx'),N('/T'):T('color'),N('/V'):T('Color text'),N('/Rect'):A(map(I,[20,120,300,170])),N('/F'):I(4)}));p[N('/Annots')]=A([field]);w._root_object[N('/AcroForm')]=D({N('/Fields'):A([field]),N('/DA'):T('/F1 24 Tf '+color),N('/NeedAppearances'):B(True),N('/DR'):D({N('/Font'):D({N('/F1'):font})})})
 with path.open('xb') as f:w.write(f)
# Pinned chromium/8044 Windows x64 FPDF_FORMFILLINFO v2. All callbacks null:
# no JavaScript platform, external actions, file access, timers or navigation.
class FormInfo(c.Structure):
 _fields_=[('version',c.c_int),('callbacks',c.c_void_p*16),('xfa_disabled',c.c_int),('extensions',c.c_void_p*17)]
def render(engine,path):
 dll=engine.dll;doc=dll.FPDF_LoadDocument(str(path).encode(),None);page=None;bitmap=None;form=None
 if not doc:raise RuntimeError('open failed')
 try:
  signatures={'FPDFDOC_InitFormFillEnvironment':(c.c_void_p,[c.c_void_p,c.c_void_p]),'FPDFDOC_ExitFormFillEnvironment':(None,[c.c_void_p]),'FORM_OnAfterLoadPage':(None,[c.c_void_p,c.c_void_p]),'FORM_OnBeforeClosePage':(None,[c.c_void_p,c.c_void_p]),'FPDF_FFLDraw':(None,[c.c_void_p,c.c_void_p,c.c_void_p,c.c_int,c.c_int,c.c_int,c.c_int,c.c_int,c.c_int])}
  for name,(result,args) in signatures.items():fn=getattr(dll,name);fn.restype=result;fn.argtypes=args
  assert c.sizeof(c.c_void_p)==8 and c.sizeof(FormInfo)==280
  assert (FormInfo.version.offset,FormInfo.callbacks.offset,FormInfo.xfa_disabled.offset,FormInfo.extensions.offset)==(0,8,136,144)
  info=FormInfo();info.version=2;info.xfa_disabled=1;form=dll.FPDFDOC_InitFormFillEnvironment(doc,c.byref(info))
  if not form:raise RuntimeError('form initialization failed')
  page=dll.FPDF_LoadPage(doc,0)
  if not page:raise RuntimeError('page failed')
  dll.FORM_OnAfterLoadPage(page,form)
  bitmap=dll.FPDFBitmap_Create(640,480,0)
  if not bitmap or not dll.FPDFBitmap_FillRect(bitmap,0,0,640,480,0xffffffff):raise RuntimeError('bitmap failed')
  dll.FPDF_RenderPageBitmap(bitmap,page,0,0,640,480,0,1)
  dll.FPDF_FFLDraw(form,bitmap,page,0,0,640,480,0,0)
  stride=dll.FPDFBitmap_GetStride(bitmap);ptr=dll.FPDFBitmap_GetBuffer(bitmap)
  if not ptr or stride<2560:raise RuntimeError('bad bitmap')
  return Image.frombytes('RGB',(640,480),c.string_at(ptr,stride*480),'raw','BGRX',stride)
 finally:
  if bitmap:dll.FPDFBitmap_Destroy(bitmap)
  if page:
   if form:dll.FORM_OnBeforeClosePage(page,form)
   dll.FPDF_ClosePage(page)
  if form:dll.FPDFDOC_ExitFormFillEnvironment(form)
  dll.FPDF_CloseDocument(doc)
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--dll',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False);engine=helpers['Pdfium'](a.dll);results=[]
 try:
  for name,source,canonical in CASES:
   images=[]
   for kind,color in [('ordered',source),('canonical',canonical)]:
    path=a.output/f'{name}-{kind}.pdf';fixture(path,color);im=render(engine,path);im.save(path.with_suffix('.png'));images.append(im)
   difference=ImageChops.difference(*images).getbbox();ink=all(helpers['has_ink'](im) for im in images);results.append(dict(name=name,passed=ink and difference is None,difference=difference,hasInk=ink));print(name,results[-1]['passed'],difference,flush=True)
 finally:engine.close()
 report=dict(passed=all(r['passed'] for r in results),cases=results,dllSha256=hashlib.sha256(a.dll.read_bytes()).hexdigest(),scriptSha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),limits=['Device color default appearances; no arbitrary page-content color-space claim.']);(a.output/'results.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8');raise SystemExit(0 if report['passed'] else 1)
if __name__=='__main__':main()
