"""Bounded research: replace retained Type3 banks with one invisible wide CID font.
No native writer or refusal guards are modified. Run with python -B -X utf8.
"""
import argparse
import ctypes as c
import hashlib
import importlib.util
import io
import json
import math
from pathlib import Path
import re
import fitz
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from pypdf import PdfReader, PdfWriter
from pypdf.generic import ArrayObject, ByteStringObject, ContentStream, DecodedStreamObject, DictionaryObject, FloatObject, NameObject, NumberObject, TextStringObject
ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--upm', type=int, default=1000)
parser.add_argument('--metric-scale', type=float, default=1)
parser.add_argument('--output', type=Path, default=ROOT/'artifacts/issue14-wide')
options = parser.parse_args()
OUT = options.output
OUT.mkdir(parents=True, exist_ok=True)
spec = importlib.util.spec_from_file_location('inspect_native', ROOT/'scripts/inspect-semantic-native.py')
ins = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ins)
def N(s): return NameObject('/'+s)
def D(**kw): return DictionaryObject({N(k): v for k,v in kw.items()})
def nums(v): return ArrayObject([FloatObject(x) for x in v])
def stream(w,b):
    s=DecodedStreamObject(); s.set_data(b); return w._add_object(s)
def build(source,target):
    assert source.resolve() != target.resolve(), 'Probe must not overwrite its retained source'
    w=PdfWriter(clone_from=source); page=w.pages[0]; fonts=page['/Resources']['/Font']
    defs=[]; codes={}
    for name,ref in list(fonts.items()):
        f=ref.get_object()
        if f['/Subtype'] != '/Type3': continue
        cm={int(a,16): bytes.fromhex(b).decode('utf-16-be') for body in re.findall(r'beginbfchar(.*?)endbfchar',f['/ToUnicode'].get_data().decode(),re.S) for a,b in re.findall(r'<([0-9A-Fa-f]+)>\s*<([0-9A-Fa-f]+)>',body)}
        code=0
        for value in f['/Encoding']['/Differences']:
            if isinstance(value,int): code=int(value); continue
            metric=list(map(float,ContentStream(f['/CharProcs'][value].get_object(),w).operations[0][0]))
            metric=[v*options.metric_scale for v in metric]; defs.append((cm[code],metric)); codes[(str(name),code)]=len(defs); code+=1
    assert 0<len(defs)<65535
    upm=options.upm; scale=upm/1000; fb=FontBuilder(upm,isTTF=True); order=['.notdef']+[f'g{i}' for i in range(1,len(defs)+1)]; fb.setupGlyphOrder(order)
    glyphs={}; metrics={}
    for name,(_,m) in zip(order,[('',[0,0,0,0,0,0])]+defs):
        width,_,x0,y0,x1,y1=m; p=TTGlyphPen(None)
        x0,y0,x1,y1=[round(v*scale) for v in (x0,y0,x1,y1)]
        assert all(-32768 <= v <= 32767 for v in (x0,y0,x1,y1)), 'Probe glyph coordinate exceeds signed 16-bit range'
        assert 0 <= round(width*scale) <= 65535, 'Probe advance exceeds unsigned 16-bit range'
        if x0<x1 and y0<y1:
            p.moveTo((x0,y0)); p.lineTo((x0,y1)); p.lineTo((x1,y1)); p.lineTo((x1,y0)); p.closePath()
        glyphs[name]=p.glyph(); metrics[name]=(max(0,round(width*scale)),x0)
    global_box=[min(m[2] for _,m in defs),min(m[3] for _,m in defs),max(m[4] for _,m in defs),max(m[5] for _,m in defs)]
    ascent=math.ceil(global_box[3]*scale); descent=math.floor(global_box[1]*scale)
    fb.setupGlyf(glyphs); fb.setupHorizontalMetrics(metrics); fb.setupHorizontalHeader(ascent=ascent,descent=descent)
    fb.setupCharacterMap({0xE000+i:name for i,name in enumerate(order[1:])})
    fb.setupNameTable({'familyName':'FolioWideProbe','styleName':'Regular','uniqueFontIdentifier':'FolioWideProbe','fullName':'FolioWideProbe','psName':'FolioWideProbe'})
    fb.setupOS2(sTypoAscender=ascent,sTypoDescender=descent,usWinAscent=max(0,ascent),usWinDescent=max(0,-descent)); fb.setupPost(); fb.setupMaxp()
    buffer=io.BytesIO(); fb.save(buffer)
    descriptor=w._add_object(D(Type=N('FontDescriptor'),FontName=N('FolioWideProbe'),Flags=NumberObject(4),FontBBox=nums(global_box),ItalicAngle=NumberObject(0),Ascent=NumberObject(math.ceil(global_box[3])),Descent=NumberObject(math.floor(global_box[1])),CapHeight=NumberObject(math.ceil(global_box[3])),StemV=NumberObject(80),FontFile2=stream(w,buffer.getvalue())))
    cid=w._add_object(D(Type=N('Font'),Subtype=N('CIDFontType2'),BaseFont=N('FolioWideProbe'),CIDSystemInfo=D(Registry=TextStringObject('Adobe'),Ordering=TextStringObject('Identity'),Supplement=NumberObject(0)),FontDescriptor=descriptor,CIDToGIDMap=stream(w,b''.join(i.to_bytes(2,'big') for i in range(len(defs)+1))),DW=NumberObject(0),W=ArrayObject([NumberObject(1),nums([m[0] for _,m in defs])])))
    cmap=['/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /Wide def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange']
    for start in range(0,len(defs),100):
        batch=defs[start:start+100]; cmap.append(f'{len(batch)} beginbfchar'); cmap.extend(f'<{start+i+1:04X}> <{ch.encode("utf-16-be").hex()}>' for i,(ch,_) in enumerate(batch)); cmap.append('endbfchar')
    cmap.append('endcmap CMapName currentdict /CMap defineresource pop end end')
    wide=w._add_object(D(Type=N('Font'),Subtype=N('Type0'),BaseFont=N('FolioWideProbe'),Encoding=N('Identity-H'),DescendantFonts=ArrayObject([cid]),ToUnicode=stream(w,'\n'.join(cmap).encode('ascii'))))
    ops=ContentStream(page.get_contents(),w).operations; tj=[]; matrix=None; size=None; bank=None
    for values,op in ops:
        if op==b'Tf': bank=str(values[0]); size=float(values[1])/options.metric_scale
        elif op==b'Tm': matrix=' '.join(map(str,values))
        elif op==b'TJ':
            for item in values[0]:
                if isinstance(item,(ByteStringObject,TextStringObject)):
                    data=bytes(item) if isinstance(item,ByteStringObject) else item.original_bytes
                    tj.append('<'+''.join(f'{codes[(bank,b)]:04X}' for b in data)+'>')
                else: tj.append(str(float(item)*options.metric_scale))
    raw=page.get_contents().get_data(); prefix=raw.split(b'BT',1)[0]
    page[N('Contents')]=stream(w,prefix+f'BT /Wide {size} Tf 3 Tr {matrix} Tm [ '.encode()+ ' '.join(tj).encode()+b' ] TJ ET\n')
    for name in list(fonts):
        if fonts[name]['/Subtype']=='/Type3': del fonts[name]
    fonts[N('Wide')]=wide; w.write(target)
    return len(defs),hashlib.sha256(prefix).hexdigest()
dllpath=ROOT/'src-tauri/resources/pdfium/pdfium.dll'; dll=c.CDLL(str(dllpath))
for name,args,result in [('FPDF_InitLibrary',[],None),('FPDF_DestroyLibrary',[],None),('FPDF_LoadDocument',[c.c_char_p,c.c_char_p],c.c_void_p),('FPDF_CloseDocument',[c.c_void_p],None),('FPDF_LoadPage',[c.c_void_p,c.c_int],c.c_void_p),('FPDF_ClosePage',[c.c_void_p],None),('FPDFText_LoadPage',[c.c_void_p],c.c_void_p),('FPDFText_ClosePage',[c.c_void_p],None),('FPDFText_CountChars',[c.c_void_p],c.c_int),('FPDFText_GetUnicode',[c.c_void_p,c.c_int],c.c_uint),('FPDFText_GetCharBox',[c.c_void_p,c.c_int]+[c.POINTER(c.c_double)]*4,c.c_int)]:
    f=getattr(dll,name); f.argtypes=args; f.restype=result
dll.FPDFText_GetText.argtypes=[c.c_void_p,c.c_int,c.c_int,c.POINTER(c.c_ushort)]
dll.FPDFText_GetText.restype=c.c_int
def pdfium(path):
    doc=page=text=None
    try:
        doc=dll.FPDF_LoadDocument(str(path).encode('utf-8'),None); assert doc
        page=dll.FPDF_LoadPage(doc,0); assert page
        text=dll.FPDFText_LoadPage(page); assert text
        result=[]
        for i in range(dll.FPDFText_CountChars(text)):
            values=[c.c_double() for _ in range(4)]; assert dll.FPDFText_GetCharBox(text,i,*[c.byref(v) for v in values])
            result.append({'text':chr(dll.FPDFText_GetUnicode(text,i)),'box':[v.value for v in values]})
        buf=(c.c_ushort * (2*len(result)+1))()
        written=dll.FPDFText_GetText(text,0,len(result),buf)
        raw=bytes(buf)[:max(0,written-1)*2].decode('utf-16-le')
        return result,raw
    finally:
        if text: dll.FPDFText_ClosePage(text)
        if page: dll.FPDF_ClosePage(page)
        if doc: dll.FPDF_CloseDocument(doc)
def measure(chars,expected):
    exact=''.join(x['text'] for x in chars).rstrip('\r\n')==''.join(x['text'] for x in expected)
    errors=[ins.box_error(a['box'],e['cluster_box']) for a,e in zip(chars,expected) if e['ink_box']] if exact else []
    return {'exact':exact,'max_ink_edge_error':max(errors,default=None),'geometry_pass':exact and bool(errors) and max(errors)<=.06}
def main():
    rows=[]; dll.FPDF_InitLibrary()
    try:
        for dirname,names in [('semantic-banks-native',None),('semantic-native',{'ligatures-on','decomposed','two-axis-marks','arabic'})]:
            folder=ROOT/'artifacts/shaped-text'/dirname
            evidence=json.loads((folder/'results.json').read_text(encoding='utf-8'))
            for entry in evidence['cases']:
                if names is not None and entry['case'] not in names: continue
                source=folder/entry['pdf']; target=OUT/source.name
                layout=json.loads(source.with_suffix('.layout.json').read_text(encoding='utf-8')); expected=ins.expected_clusters(layout)
                count,prefix=build(source,target); pc,praw=pdfium(target); original_pc,oraw=pdfium(source); mu=[]
                with fitz.open(target) as doc:
                    page=doc[0]; height=float(PdfReader(target).pages[0].mediabox.top)
                    for b in page.get_text('rawdict')['blocks']:
                        for line in b.get('lines',[]):
                            for span in line['spans']:
                                for ch in span['chars']:
                                    x0,y0,x1,y1=ch['bbox']; mu.append({'text':ch['c'],'box':[x0,x1,height-y1,height-y0]})
                    raster=hashlib.sha256(page.get_pixmap().samples).hexdigest()
                with fitz.open(source) as doc: original_raster=hashlib.sha256(doc[0].get_pixmap().samples).hexdigest()
                raw={'pdfium':praw,'mupdf':''.join(x['text'] for x in mu),'pypdf':PdfReader(target).pages[0].extract_text()}
                row=dict(case=entry['case'],rotation=entry['rotation'],definitions=count,source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),output_sha256=hashlib.sha256(target.read_bytes()).hexdigest(),visible_prefix_sha256=prefix,source=layout['text'],raw=raw,pdfium=measure(pc,expected),mupdf=measure(mu,expected),raster_unchanged=raster==original_raster,characters={'pdfium':pc,'mupdf':mu},expected=expected)
                row['original_pdfium']=measure(original_pc,expected)
                row['original_pdfium_raw']=oraw
                row['original_pdfium_characters']=original_pc
                row['exact_all_readers']=all(t.rstrip('\r\n')==layout['text'] for t in raw.values()); rows.append(row)
                print(row['case'],row['rotation'],count,row['exact_all_readers'],row['pdfium'],flush=True)
    finally: dll.FPDF_DestroyLibrary()
    report={'design':'Single Type0 Identity-H + CIDFontType2 synthetic rectangle glyphs, explicit CIDToGIDMap, per-code ToUnicode, 3 Tr, one TJ; original widths/adjustments and visible prefix preserved. Configurable UPM and metric enlargement, inverse font size and enlarged TJ adjustments.','pdfium_sha256':hashlib.sha256(dllpath.read_bytes()).hexdigest(),'upm':options.upm,'metric_scale':options.metric_scale,'tolerance_points':.06,'cases':rows}
    (OUT/'results.json').write_text(json.dumps(report,ensure_ascii=True,indent=2),encoding='utf-8')
if __name__=='__main__': main()
