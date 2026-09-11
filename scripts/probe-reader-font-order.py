"""Small, synthetic PDF text-object ordering controls. No source documents are edited."""
import argparse
import ctypes as c
import hashlib
import importlib.metadata
import json
from pathlib import Path
import fitz
from pypdf import PdfWriter, PdfReader
from pypdf.generic import DictionaryObject as D, NameObject as N, NumberObject as I, ArrayObject as A, DecodedStreamObject as S
ROOT = Path(__file__).resolve().parents[1]

def stream(data):
    obj=S(); obj.set_data(data); return obj

def font(writer, kind):
    if kind == 'courier':
        return writer._add_object(D({N('/Type'):N('/Font'),N('/Subtype'):N('/Type1'),N('/BaseFont'):N('/Courier')}))
    procs=D(); differences=A([I(65)])
    for ch in 'ABCDEF':
        name=N('/g'+ch); differences.append(name)
        procs[name]=writer._add_object(stream(b'600 0 0 0 600 800 d1\n'))
    cmap=b'/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /Order def /CMapType 2 def 1 begincodespacerange <00> <FF> endcodespacerange\n6 beginbfchar\n'+b''.join(f'<{ord(ch):02X}> <{ord(ch):04X}>\n'.encode() for ch in 'ABCDEF')+b'endbfchar endcmap CMapName currentdict /CMap defineresource pop end end'
    from pypdf.generic import FloatObject as F
    return writer._add_object(D({N('/Type'):N('/Font'),N('/Subtype'):N('/Type3'),N('/FontBBox'):A(map(I,[0,0,600,800])),N('/FontMatrix'):A(map(F,[.001,0,0,.001,0,0])),N('/CharProcs'):procs,N('/Encoding'):D({N('/Type'):N('/Encoding'),N('/Differences'):differences}),N('/FirstChar'):I(65),N('/LastChar'):I(70),N('/Widths'):A([I(600)]*6),N('/Resources'):D(),N('/ToUnicode'):writer._add_object(stream(cmap))}))

CONTENTS={
 'single':b'BT /F0 20 Tf 1 0 0 1 48 70 Tm [(ABCDEF)] TJ ET',
 'split-tj':b'BT /F0 20 Tf 1 0 0 1 48 70 Tm [(AB)] TJ [(CD)] TJ [(EF)] TJ ET',
 'same-tf':b'BT /F0 20 Tf 1 0 0 1 48 70 Tm [(AB)] TJ /F0 20 Tf [(CD)] TJ /F0 20 Tf [(EF)] TJ ET',
 'alias-tf':b'BT /F0 20 Tf 1 0 0 1 48 70 Tm [(AB)] TJ /F1 20 Tf [(CD)] TJ /F0 20 Tf [(EF)] TJ ET',
 'distinct-tf':b'BT /F0 20 Tf 1 0 0 1 48 70 Tm [(AB)] TJ /F1 20 Tf [(CD)] TJ /F0 20 Tf [(EF)] TJ ET',
 'distinct-tm':b'BT /F0 20 Tf 1 0 0 1 48 70 Tm [(AB)] TJ /F1 20 Tf 1 0 0 1 72 70 Tm [(CD)] TJ /F0 20 Tf 1 0 0 1 96 70 Tm [(EF)] TJ ET',
}

def native_reader(dll_path):
    dll=c.CDLL(str(dll_path))
    for name,args,result in [
      ('FPDF_InitLibrary',[],None),('FPDF_DestroyLibrary',[],None),('FPDF_LoadDocument',[c.c_char_p,c.c_char_p],c.c_void_p),('FPDF_CloseDocument',[c.c_void_p],None),('FPDF_LoadPage',[c.c_void_p,c.c_int],c.c_void_p),('FPDF_ClosePage',[c.c_void_p],None),('FPDFText_LoadPage',[c.c_void_p],c.c_void_p),('FPDFText_ClosePage',[c.c_void_p],None),('FPDFText_CountChars',[c.c_void_p],c.c_int),('FPDFText_GetUnicode',[c.c_void_p,c.c_int],c.c_uint),('FPDFText_GetCharBox',[c.c_void_p,c.c_int]+[c.POINTER(c.c_double)]*4,c.c_int),('FPDFText_GetTextObject',[c.c_void_p,c.c_int],c.c_void_p),('FPDFPage_CountObjects',[c.c_void_p],c.c_int),('FPDFPage_SetRotation',[c.c_void_p,c.c_int],None)]:
        fn=getattr(dll,name); fn.argtypes=args; fn.restype=result
    return dll

def inspect(dll,path,neutralize=False):
    doc=page=text=None
    try:
        doc=dll.FPDF_LoadDocument(str(path).encode(),None)
        if not doc: raise RuntimeError('PDF load failed')
        page=dll.FPDF_LoadPage(doc,0)
        if not page: raise RuntimeError('Page load failed')
        if neutralize: dll.FPDFPage_SetRotation(page,0)
        text=dll.FPDFText_LoadPage(page)
        if not page or not text: raise RuntimeError('Text load failed')
        chars=[]; objects={}
        for i in range(dll.FPDFText_CountChars(text)):
            box=[c.c_double() for _ in range(4)]
            if not dll.FPDFText_GetCharBox(text,i,*map(c.byref,box)): raise RuntimeError('Missing box')
            obj=dll.FPDFText_GetTextObject(text,i)
            if obj not in objects: objects[obj]=len(objects)
            chars.append(dict(text=chr(dll.FPDFText_GetUnicode(text,i)),box=[n.value for n in box],textObject=objects[obj]))
        return dict(text=''.join(ch['text'] for ch in chars),characters=chars,pageObjectCount=dll.FPDFPage_CountObjects(page))
    finally:
        if text: dll.FPDFText_ClosePage(text)
        if page: dll.FPDF_ClosePage(page)
        if doc: dll.FPDF_CloseDocument(doc)

def main():
    parser=argparse.ArgumentParser(description=__doc__); parser.add_argument('--output',type=Path,required=True); parser.add_argument('--counterrotation',action='store_true'); args=parser.parse_args()
    cases = {'counterrotated': b'BT /F0 20 Tf -1 0 0 -1 220 100 Tm [(AB)] TJ [(CD)] TJ [(EF)] TJ ET'} if args.counterrotation else CONTENTS
    args.output.mkdir(parents=True,exist_ok=False)
    dll_path=ROOT/'src-tauri/resources/pdfium/pdfium.dll'; dll=native_reader(dll_path); dll.FPDF_InitLibrary(); rows=[]
    try:
        for kind in ['courier','type3']:
            for form,content in cases.items():
                for rotation in [0,90,180,270]:
                    writer=PdfWriter(); page=writer.add_blank_page(width=300,height=160); first=font(writer,kind); second=first if form=='alias-tf' else font(writer,kind)
                    page[N('/Resources')]=D({N('/Font'):D({N('/F0'):first,N('/F1'):second})}); page[N('/Rotate')]=I(rotation); page[N('/Contents')]=writer._add_object(stream(content))
                    path=args.output/f'{kind}-{form}-{rotation}.pdf'; writer.write(path)
                    row=dict(kind=kind,form=form,rotation=rotation,pdf=path.name,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),pdfium=inspect(dll,path))
                    row['neutralized']=inspect(dll,path,True)
                    with fitz.open(path) as pdf: row['mupdf']=pdf[0].get_text()
                    row['pypdf']=PdfReader(path).pages[0].extract_text(); rows.append(row)
        report=dict(source='ABCDEF',pdfiumSha256=hashlib.sha256(dll_path.read_bytes()).hexdigest(),versions={p:importlib.metadata.version(p) for p in ['pymupdf','pypdf']},cases=rows)
        (args.output/'results.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
        for kind in ['courier','type3']:
            for form in cases:
                group=[r for r in rows if r['kind']==kind and r['form']==form]
                print(kind,form,[(r['rotation'],r['pdfium']['text'],r['pdfium']['pageObjectCount']) for r in group])
    finally: dll.FPDF_DestroyLibrary()
if __name__=='__main__': main()
