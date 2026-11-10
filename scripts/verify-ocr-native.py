"""Independent reader and pixel checks for the opt-in native OCR transport fixtures."""
from pathlib import Path
import ctypes, json, re
import fitz
from pypdf import PdfReader

root=Path(__file__).resolve().parents[1]
directory=root/'artifacts/pdf-ocr-native'
dll=ctypes.CDLL(str(root/'src-tauri/resources/pdfium/pdfium.dll'))
dll.FPDF_InitLibrary()
for name,args,result in [
    ('FPDF_LoadDocument',[ctypes.c_char_p,ctypes.c_char_p],ctypes.c_void_p),
    ('FPDF_LoadPage',[ctypes.c_void_p,ctypes.c_int],ctypes.c_void_p),
    ('FPDFText_LoadPage',[ctypes.c_void_p],ctypes.c_void_p),
    ('FPDFText_CountChars',[ctypes.c_void_p],ctypes.c_int),
    ('FPDFText_GetText',[ctypes.c_void_p,ctypes.c_int,ctypes.c_int,ctypes.c_void_p],ctypes.c_int),
    ('FPDFText_GetUnicode',[ctypes.c_void_p,ctypes.c_int],ctypes.c_uint),
    ('FPDFText_GetCharBox',[ctypes.c_void_p,ctypes.c_int]+[ctypes.POINTER(ctypes.c_double)]*4,ctypes.c_int),
    ('FPDFText_ClosePage',[ctypes.c_void_p],None),('FPDF_ClosePage',[ctypes.c_void_p],None),
    ('FPDF_CloseDocument',[ctypes.c_void_p],None)]:
    function=getattr(dll,name);function.argtypes=args;function.restype=result

def pdfium_read(path,rotation):
    document=dll.FPDF_LoadDocument(str(path).encode(),None)
    assert document
    page=dll.FPDF_LoadPage(document,0);assert page
    text=dll.FPDFText_LoadPage(page);assert text
    try:
        count=dll.FPDFText_CountChars(text);assert 0<count<10000
        buffer=ctypes.create_string_buffer((count+1)*4)
        size=dll.FPDFText_GetText(text,0,count,buffer)
        decoded=buffer.raw[:(size-1)*2].decode('utf-16le')
        indexed=''.join(chr(dll.FPDFText_GetUnicode(text,i)) for i in range(count))
        def point(x,y):
            return {0:(x-20,130-y),90:(y-30,x-20),180:(220-x,y-30),270:(130-y,220-x)}[rotation]
        boxes={}
        for word in ['café','niño']:
            index=indexed.index(word);points=[]
            for i in range(index,index+len(word)):
                left,right,bottom,top=[ctypes.c_double() for _ in range(4)]
                assert dll.FPDFText_GetCharBox(text,i,ctypes.byref(left),ctypes.byref(right),ctypes.byref(bottom),ctypes.byref(top))
                points.extend([point(left.value,bottom.value),point(right.value,top.value)])
            boxes[word]=[min(p[0] for p in points),min(p[1] for p in points),max(p[0] for p in points),max(p[1] for p in points)]
        return decoded,boxes
    finally:
        dll.FPDFText_ClosePage(text);dll.FPDF_ClosePage(page);dll.FPDF_CloseDocument(document)

results=[]
for fixture in json.loads((directory/'fixtures.json').read_text(encoding='utf-8')):
    before=fitz.open(fixture['before']);after=fitz.open(fixture['after'])
    try:
        pdfium_text,pdfium_boxes=pdfium_read(fixture['after'],(fixture['intrinsic']+fixture['extra'])%360)
        texts={'mupdf':after[0].get_text(),'pypdf':PdfReader(fixture['after']).pages[0].extract_text(),'pdfium':pdfium_text}
        pixels_equal=before[0].get_pixmap(matrix=fitz.Matrix(2,2),alpha=False).samples==after[0].get_pixmap(matrix=fitz.Matrix(2,2),alpha=False).samples
        ordered={name:re.sub(r'\s+',' ',text.replace('Native header','')).strip()=='café niño 𐐀 final' for name,text in texts.items()}
        # MuPDF returns unrotated coordinates; selections must cover the scan words
        # in displayed coordinates, not merely extract the right Unicode string.
        boxes={word[4]:list(fitz.Rect(word[:4])*after[0].rotation_matrix) for word in after[0].get_text('words')}
        expected={'café':[10,30,46,46],'niño':[54,30,88,46]}
        geometry=all(text in boxes and max(abs(a-b) for a,b in zip(boxes[text],bounds))<0.1 for text,bounds in expected.items())
        # Independent readers need usable selection boxes, not identical glyph
        # metrics. PDFium's tight boxes extend ~0.24 pt above these 16 pt words;
        # allow at most half a point and retain the measured error in the report.
        pdfium_error=max(abs(a-b) for text,bounds in expected.items() for a,b in zip(pdfium_boxes[text],bounds))
        pdfium_geometry=pdfium_error<=0.5
        results.append({'intrinsic':fixture['intrinsic'],'extra':fixture['extra'],'pixelsEqual':pixels_equal,
                        'ocrOrder':ordered,'texts':texts,'mupdfGeometry':geometry,'wordBoxes':boxes,'pdfiumGeometry':pdfium_geometry,'pdfiumWordBoxes':pdfium_boxes,'pdfiumGeometryMaxError':pdfium_error,
                        'headerPreserved':all('Native header' in value for value in texts.values())})
    finally:before.close();after.close()
(directory/'verification.json').write_text(json.dumps(results,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps({'fixtures':len(results),'pixelPasses':sum(r['pixelsEqual'] for r in results),
    'readerOrderPasses':{name:sum(r['ocrOrder'][name] for r in results) for name in ['mupdf','pypdf','pdfium']},
    'headerPasses':sum(r['headerPreserved'] for r in results),'mupdfGeometryPasses':sum(r['mupdfGeometry'] for r in results),
    'pdfiumGeometryPasses':sum(r['pdfiumGeometry'] for r in results),'pdfiumGeometryMaxError':max(r['pdfiumGeometryMaxError'] for r in results)}))
assert all(r['pixelsEqual'] and r['headerPreserved'] and r['mupdfGeometry'] and r['pdfiumGeometry'] and all(r['ocrOrder'].values()) for r in results)
