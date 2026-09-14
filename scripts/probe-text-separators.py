"""Experimental separator encodings; never used by the production writer.

All candidates draw identical original vector ink. Only the invisible semantic
layer changes. Exact copying and per-character boxes are independent gates.
"""
import argparse, ctypes as c, hashlib, importlib.util, json
from pathlib import Path
import fitz
from pypdf import PdfWriter, PdfReader
from pypdf.generic import DictionaryObject as D, NameObject as N, NumberObject as I, FloatObject as F, ArrayObject as A
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('reader',ROOT/'scripts/probe-reader-font-order.py')
reader=importlib.util.module_from_spec(spec);spec.loader.exec_module(reader)


def cells_for(text, wrap=False):
    cells=[]; column=0; row=0
    for index,char in enumerate(text):
        if wrap and index==4: row+=1; column=0
        cells.append({'text':char,'x':48+column*12,'y':120-row*28,'width':12})
        if char=='\n': row+=1;column=0
        else: column+=1
    return cells


def visible(cells):
    # Original small vector letter specimen: no extractable duplicate text.
    paths={'A':'0 0 m 4 14 l 8 0 l 6 0 l 5 4 l 3 4 l 2 0 l h 3.5 6 m 4.5 6 l 4 9 l h f*',
           'B':'0 0 2 14 re 2 0 6 2 re 2 6 5 2 re 2 12 6 2 re 6 2 2 4 re 6 8 2 4 re f',
           'C':'0 0 2 14 re 2 0 6 2 re 2 12 6 2 re f',
           'D':'0 0 2 14 re 2 0 4 2 re 2 12 4 2 re 6 2 2 10 re f'}
    return '\n'.join(f"q 1 0 0 1 {v['x']+2} {v['y']} cm {paths[v['text']]} Q" for v in cells if v['text'] in paths).encode()


def expanded(cells):
    result=[]
    for cell in cells:
        if result and cell['text']==' ' and result[-1]['text'].strip(' ')=='' and cell['y']==result[-1]['y']:
            result[-1]['text']+=' '; result[-1]['width']+=cell['width']
        else: result.append(dict(cell))
    return result


def type3(writer,cells):
    procs=D();diff=A([I(1)]);widths=A();mappings=[]
    for index,cell in enumerate(cells,1):
        name=N(f'/g{index}');diff.append(name);width=cell['width']*50;dy=(cell['y']-120)*50
        procs[name]=writer._add_object(reader.stream(f'{width} 0 0 {dy-200} {width} {dy+800} d1\n'.encode()))
        widths.append(F(width)); unicode=cell['text'].encode('utf-16-be').hex().upper()
        mappings.append(f'<{index:02X}> <{unicode}>')
    cmap='/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /Separators def /CMapType 2 def 1 begincodespacerange <00> <FF> endcodespacerange\n'+str(len(cells))+' beginbfchar\n'+'\n'.join(mappings)+'\nendbfchar endcmap CMapName currentdict /CMap defineresource pop end end'
    return writer._add_object(D({N('/Type'):N('/Font'),N('/Subtype'):N('/Type3'),N('/FontBBox'):A(map(I,[0,-3000,6000,800])),N('/FontMatrix'):A(map(F,[.001,0,0,.001,0,0])),N('/CharProcs'):procs,N('/Encoding'):D({N('/Type'):N('/Encoding'),N('/Differences'):diff}),N('/FirstChar'):I(1),N('/LastChar'):I(len(cells)),N('/Widths'):widths,N('/Resources'):D(),N('/ToUnicode'):writer._add_object(reader.stream(cmap.encode()))}))


def write_case(path,text,wrap,strategy,rotation):
    original=cells_for(text,wrap);cells=expanded(original) if strategy=='expanded-unicode' else original
    writer=PdfWriter();page=writer.add_blank_page(width=240,height=180);page[N('/Rotate')]=I(rotation)
    ink=visible(original)
    if strategy=='courier-objects':
        font=reader.font(writer,'courier');semantic=[]
        for cell in cells:
            code=ord(cell['text']);semantic.append(f"BT /S 20 Tf 3 Tr 1 0 0 1 {cell['x']} {cell['y']} Tm <{code:02X}> Tj ET")
        semantic='\n'.join(semantic)
    else:
        font=type3(writer,cells);ops=[]
        for index,cell in enumerate(cells):
            ops.append(f'<{index+1:02X}>')
            if index+1<len(cells):ops.append(str((cell['x']+cell['width']-cells[index+1]['x'])*50))
        semantic='BT /S 20 Tf 1 0 0 1 48 120 Tm ['+' '.join(ops)+'] TJ ET'
        if strategy=='whole-actualtext':semantic=f"/Span << /ActualText <FEFF{text.encode('utf-16-be').hex()}> >> BDC\n{semantic}\nEMC"
    page[N('/Resources')]=D({N('/Font'):D({N('/S'):font})})
    page[N('/Contents')]=writer._add_object(reader.stream(ink+b'\n'+semantic.encode()))
    writer.write(path)
    return original


def exact_boxes(actual,cells):
    # Raw PDFium uses unrotated PDF coordinates. Require a scalar per source cell,
    # exact order and tight known cell bounds; do not accept union-box expansion.
    if len(actual)!=len(cells):return False
    return all(a['text']==b['text'] and max(abs(x-y) for x,y in zip(a['box'],[b['x'],b['x']+b['width'],b['y']-4,b['y']+16]))<=.02 for a,b in zip(actual,cells))


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);args=p.parse_args();args.output.mkdir(parents=True,exist_ok=False)
    dllpath=ROOT/'src-tauri/resources/pdfium/pdfium.dll';dll=reader.native_reader(dllpath)
    dll.FPDFText_GetText.argtypes=[c.c_void_p,c.c_int,c.c_int,c.POINTER(c.c_ushort)];dll.FPDFText_GetText.restype=c.c_int
    dll.FPDF_InitLibrary();rows=[]
    try:
        for name,text,wrap in [('single','A B',False),('spaces','A  B',False),('soft','A B C D',True),('soft-spaces','A  B C D',True),('hard','A\nB',False)]:
            for rotation in [0,90,180,270]:
                reference=None
                for strategy in ['courier-objects','scalar-cells','expanded-unicode','whole-actualtext']:
                    path=args.output/f'{name}-{rotation}-{strategy}.pdf';cells=write_case(path,text,wrap,strategy,rotation)
                    raw=reader.inspect(dll,path)
                    doc=dll.FPDF_LoadDocument(str(path).encode(),None);page=dll.FPDF_LoadPage(doc,0);tp=dll.FPDFText_LoadPage(page)
                    count=dll.FPDFText_CountChars(tp);buf=(c.c_ushort*(count+1))();length=dll.FPDFText_GetText(tp,0,count,buf)
                    extracted=bytes(buf)[:max(0,length-1)*2].decode('utf-16-le');dll.FPDFText_ClosePage(tp);dll.FPDF_ClosePage(page);dll.FPDF_CloseDocument(doc)
                    with fitz.open(path) as pdf:
                        mu=pdf[0].get_text();pix=pdf[0].get_pixmap(matrix=fitz.Matrix(2,2),alpha=False);pixels=(pix.width,pix.height,pix.samples)
                    if reference is None:reference=pixels
                    py=PdfReader(path).pages[0].extract_text()
                    texts={'pdfium':extracted,'mupdf':mu,'pypdf':py}
                    rows.append({'case':name,'strategy':strategy,'rotation':rotation,'expected':text,'texts':texts,'exact':{k:v==text for k,v in texts.items()},'terminalEolOnly':{k:v in [text,text+'\n',text+'\r\n'] for k,v in texts.items()},'pdfiumCharacters':raw['characters'],'pdfiumCellBoxesExact':exact_boxes(raw['characters'],cells),'unchangedInk':reference==pixels,'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'pdf':path.name})
    finally:dll.FPDF_DestroyLibrary()
    report={'description':'Experimental only. Exact text, optional terminal-EOL diagnostic, character boxes and original vector ink are separate gates. No production writer changed.','pdfiumSha256':hashlib.sha256(dllpath.read_bytes()).hexdigest(),'mupdf':fitz.VersionBind,'pypdf':__import__('pypdf').__version__,'cases':rows}
    (args.output/'results.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    assert len(rows)==80 and all(r['unchangedInk'] for r in rows)
    for strategy in ['courier-objects','scalar-cells','expanded-unicode','whole-actualtext']:
        group=[r for r in rows if r['strategy']==strategy]
        print(strategy,'exact',sum(all(r['exact'].values()) for r in group),'terminalOnly',sum(all(r['terminalEolOnly'].values()) for r in group),'geometry',sum(r['pdfiumCellBoxesExact'] for r in group),'of',len(group))

if __name__=='__main__':main()
