"""Direct bounded C API List selection/save/reopen and independent paint check.

Synthetic options have literal expected indices. No Folio code or application
outputs enter the expectations. Each save is cold reopened. A selected empty
export must paint just like any other selected row; empty selection must not.
"""
import argparse
import ctypes as c
import hashlib
import json
from pathlib import Path
import runpy

import fitz
from PIL import Image
from pypdf import PdfReader, PdfWriter
from pypdf.generic import (ArrayObject as A, BooleanObject as B, ContentStream,
                          DecodedStreamObject, DictionaryObject as D, NameObject as N,
                          NumberObject as I, TextStringObject as T)

HELPERS=runpy.run_path(str(Path(__file__).with_name('probe-pdfium-form-fonts.py')))
ROOT=Path(__file__).resolve().parent.parent
RECT=[100,165,350,235]
OPTIONS=['Red','Green','Blue']

def sha(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def fixture(path, options, multi=False, top=0, initial=None):
    """Independent synthetic field, no inherited application-generated AP."""
    w=PdfWriter();p=w.add_blank_page(450,300)
    font=w._add_object(D({N('/Type'):N('/Font'),N('/Subtype'):N('/Type1'),N('/BaseFont'):N('/Helvetica')}))
    def opt(v): return A([T(s) for s in v]) if isinstance(v,list) else T(v)
    f=D({N('/Type'):N('/Annot'),N('/Subtype'):N('/Widget'),N('/FT'):N('/Ch'),
         N('/T'):T('Answer'),N('/Rect'):A(map(I,RECT)),N('/F'):I(4),
         N('/Ff'):I((1<<21) if multi else 0),N('/Opt'):A([opt(v) for v in options]),
         N('/TI'):I(top),N('/DA'):T('/Helv 14 Tf 0 g'),
         N('/BS'):D({N('/S'):N('/S'),N('/W'):I(1)}),N('/MK'):D({N('/BC'):A([I(0)]),N('/BG'):A([I(1)])})})
    if initial is not None:
        # Literal fallback-policy inputs, not normalized by the test.
        for k,v in initial.items():
            f[N(k)]=A(map(I,v)) if k=='/I' else T(v)
    ref=w._add_object(f);p[N('/Annots')]=A([ref])
    w._root_object[N('/AcroForm')]=D({N('/Fields'):A([ref]),N('/NeedAppearances'):B(True),
        N('/DA'):T('/Helv 14 Tf 0 g'),N('/DR'):D({N('/Font'):D({N('/Helv'):font})})})
    with path.open('xb') as out:w.write(out)

def signatures(dll):
    specs={
        'FPDFDOC_InitFormFillEnvironment':(c.c_void_p,[c.c_void_p,c.c_void_p]),
        'FPDFDOC_ExitFormFillEnvironment':(None,[c.c_void_p]),
        'FORM_OnAfterLoadPage':(None,[c.c_void_p,c.c_void_p]),
        'FORM_OnBeforeClosePage':(None,[c.c_void_p,c.c_void_p]),
        'FPDF_SaveAsCopy':(c.c_int,[c.c_void_p,c.c_void_p,c.c_ulong]),
        'FPDFPage_GetAnnot':(c.c_void_p,[c.c_void_p,c.c_int]),
        'FPDFPage_CloseAnnot':(None,[c.c_void_p]),
        'FPDFAnnot_GetOptionCount':(c.c_int,[c.c_void_p,c.c_void_p]),
        'FPDFAnnot_IsOptionSelected':(c.c_int,[c.c_void_p,c.c_void_p,c.c_int]),
        'FORM_SetFocusedAnnot':(c.c_int,[c.c_void_p,c.c_void_p]),
        'FORM_SetIndexSelected':(c.c_int,[c.c_void_p,c.c_void_p,c.c_int,c.c_int]),
        'FORM_ForceToKillFocus':(c.c_int,[c.c_void_p])}
    for name,(result,args) in specs.items():
        fn=getattr(dll,name);fn.restype=result;fn.argtypes=args

def native(engine, source, target=None, selected=None):
    dll=engine.dll;signatures(dll)
    assert c.sizeof(c.c_void_p)==8 and c.sizeof(HELPERS['FormInfo'])==280
    assert HELPERS['FormInfo'].xfa_disabled.offset==136
    info=HELPERS['FormInfo']();info.version=2;info.xfa_disabled=1
    doc=dll.FPDF_LoadDocument(str(source.resolve()).encode('utf-8'),None)
    if not doc:raise RuntimeError('native open failed')
    form=page=annot=None;chunks=bytearray();failed=False
    @HELPERS['WRITE']
    def write(_,data,size):
        nonlocal failed
        try:
            if failed or not data or size>32*1024*1024-len(chunks):
                failed=True;return 0
            chunks.extend(c.string_at(data,size));return 1
        except Exception:
            failed=True;return 0
    try:
        form=dll.FPDFDOC_InitFormFillEnvironment(doc,c.byref(info))
        if not form:raise RuntimeError('native form open failed')
        page=dll.FPDF_LoadPage(doc,0)
        if not page:raise RuntimeError('native page open failed')
        dll.FORM_OnAfterLoadPage(page,form)
        annot=dll.FPDFPage_GetAnnot(page,0)
        if not annot:raise RuntimeError('native annotation open failed')
        count=dll.FPDFAnnot_GetOptionCount(form,annot)
        assert 0<count<=32,('unbounded option count',count)
        before=[i for i in range(count) if dll.FPDFAnnot_IsOptionSelected(form,annot,i)]
        if selected is not None:
            if not dll.FORM_SetFocusedAnnot(form,annot):raise RuntimeError('focus failed')
            for i in range(count):
                if not dll.FORM_SetIndexSelected(form,page,i,0):raise RuntimeError('clear failed')
            for i in selected:
                if not dll.FORM_SetIndexSelected(form,page,i,1):raise RuntimeError('select failed')
            if not dll.FORM_ForceToKillFocus(form):raise RuntimeError('blur failed')
        after=[i for i in range(count) if dll.FPDFAnnot_IsOptionSelected(form,annot,i)]
        if target is not None:
            writer=HELPERS['FileWrite'](1,write)
            if not dll.FPDF_SaveAsCopy(doc,c.byref(writer),0) or failed:raise RuntimeError('bounded save failed')
            with target.open('xb') as output:output.write(chunks)
        return dict(before=before,after=after)
    finally:
        if annot:dll.FPDFPage_CloseAnnot(annot)
        if page:
            if form:dll.FORM_OnBeforeClosePage(page,form)
            dll.FPDF_ClosePage(page)
        if form:dll.FPDFDOC_ExitFormFillEnvironment(form)
        dll.FPDF_CloseDocument(doc)

def paint(path, expected, options, output):
    r=PdfReader(path);f=r.pages[0]['/Annots'][0].get_object()
    raw_options=list(f['/Opt']);assert raw_options==options,('options changed',raw_options,options)
    top=int(f.get('/TI',0));assert 0<=top<len(options)
    ap=f['/AP']['/N'];ops=ContentStream(ap,r).operations
    labels=[str(a[0]) for a,op in ops if op==b'Tj']
    want_labels=[v[1] if isinstance(v,list) else v for v in options[top:]]
    assert labels==want_labels,('display labels changed',labels,want_labels)
    # Sample at a fixed source-owned horizontal location far from all glyphs.
    # Native Helvetica14 row height is 15.624001. These are literal row centers,
    # not values derived from the generated selection paint under test.
    # MuPDF may regenerate widgets when NeedAppearances is true. Draw the exact
    # saved AP as an ordinary Form XObject instead; never let the independent
    # reader synthesize a replacement and thereby mask/introduce a paint bug.
    w=PdfWriter(clone_from=path);page=w.pages[0]
    saved_ap=page['/Annots'][0].get_object()['/AP']['/N']
    assert list(saved_ap['/BBox'])==[0,0,250,70]
    assert list(saved_ap.get('/Matrix',[1,0,0,1,0,0]))==[1,0,0,1,0,0]
    stream=DecodedStreamObject();stream.set_data(b'q 1 0 0 1 100 165 cm /SavedAP Do Q')
    page[N('/Contents')]=w._add_object(stream)
    page[N('/Resources')]=D({N('/XObject'):D({N('/SavedAP'):w._add_object(saved_ap)})})
    page.pop('/Annots');w._root_object.pop('/AcroForm')
    flattened=output.with_suffix('.pdf')
    # Output filenames differ from the saved source PDF.
    flattened=output.with_name(output.stem+'-appearance.pdf')
    with flattened.open('xb') as out:w.write(out)
    with fitz.open(flattened) as doc:
        pix=doc[0].get_pixmap(matrix=fitz.Matrix(3,3),alpha=False,annots=True)
        im=Image.frombytes('RGB',[pix.width,pix.height],pix.samples);im.save(output)
    actual=[];samples=[]
    for row in range(min(4,len(options)-top)):
        y=65+1+(row+.5)*15.624001
        rgb=im.getpixel((330*3,round(y*3)));samples.append(rgb)
        if rgb[0]<10 and 40<=rgb[1]<=60 and 100<=rgb[2]<=125:actual.append(top+row)
        else:assert rgb==(255,255,255),('unexpected row fill',row,rgb)
    visible=[i for i in expected if top<=i<top+4]
    assert actual==visible,('selected rows have wrong paint',actual,visible,samples)
    # Border and outside remain independently known, away from selection/glyphs.
    assert im.getpixel((100*3,100*3))==(0,0,0),'source border lost'
    assert im.getpixel((99*3-2,100*3))==(255,255,255),'outside field changed'
    return dict(rawValue=f.get('/V'),rawIndices=f.get('/I'),top=top,labels=labels,
                painted=actual,rowSamples=samples,appearanceSha256=hashlib.sha256(ap.get_data()).hexdigest())

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--dll',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    args.output.mkdir(parents=True,exist_ok=False)
    engine=HELPERS['HELPERS']['Pdfium'](args.dll);results=[]
    cases=[('ordinary',OPTIONS,False,0,None,[[1],[],[0],[2],[],[0]]),
           ('empty-export',[['','Red'],'Green','Blue'],False,0,None,[[1],[],[0],[2],[],[0]]),
           ('multi-empty',[['','Red'],'Green','Blue'],True,0,None,[[0,2],[],[0],[1,2],[],[0,1]]),
           ('duplicate-exports',[['same','Red'],['same','Green'],'Blue'],False,0,None,[[1],[],[0],[2],[1]]),
           ('scrolled',['Hidden',*OPTIONS],True,1,None,[[2],[],[1],[2,3]]),
           ('value-fallback',OPTIONS,False,0,{'/V':'Green'},[[1],[2]]),
           ('invalid-index-fallback',OPTIONS,False,0,{'/V':'Green','/I':[99]},[[1],[2]])]
    try:
        for name,options,multi,top,initial,sequence in cases:
            folder=args.output/name;folder.mkdir();source=folder/'source.pdf'
            fixture(source,options,multi,top,initial)
            before=native(engine,source)
            if initial is not None:assert before['before']==[1],('native fallback policy',name,before)
            for step,indices in enumerate(sequence):
                saved=folder/f'{step:02}.pdf';row=dict(case=name,step=step,expected=indices,sourceSha256=sha(source))
                try:
                    row['api']=native(engine,source,saved,indices)
                    row['coldReopen']=native(engine,saved)
                    row['outputSha256']=sha(saved)
                    assert row['coldReopen']['before']==indices,('native model selection differs',row['coldReopen'],indices)
                    row['paint']=paint(saved,indices,options,folder/f'{step:02}.png')
                    row['passed']=True
                except (AssertionError,RuntimeError) as e:
                    row.update(passed=False,error=str(e))
                results.append(row);print(name,step,row['passed'],row.get('error',''),flush=True)
                if not saved.exists():break
                source=saved
    finally:engine.close()
    report=dict(passed=all(row['passed'] for row in results),cases=results,
        dllSha256=sha(args.dll),scriptSha256=sha(__file__),helperSha256={p:sha(ROOT/'scripts'/p)
            for p in ['probe-pdfium-form-fonts.py','probe-pdfium-form-inheritance.py']},
        limits=['Direct C API generated synthetic fields; no desktop/CLI acceptance claim',
                'Independent MuPDF fixed row-center samples and raw AP labels; not a full-page fidelity proof'])
    (args.output/'results.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    raise SystemExit(0 if report['passed'] else 1)

if __name__=='__main__':main()
