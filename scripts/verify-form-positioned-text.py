"""Independent PDF readers for nested TJ edits; no production helper imports."""
from pathlib import Path
import hashlib,json,math,os
import fitz
from PIL import Image,ImageChops,ImageDraw
from pypdf import PdfReader
from pypdf.generic import ContentStream,TextStringObject,ByteStringObject

root=Path(os.environ.get('FOLIO_FORM_POSITIONED_ARTIFACTS','artifacts/form-positioned-20261024/native'))
identity=[[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]]
def matrix(values):
    a,b,c,d,e,f=map(float,values);return [[a,c,e],[b,d,f],[0.,0.,1.]]
def multiply(a,b):return [[sum(a[i][k]*b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]
def is_string(value):return isinstance(value,(TextStringObject,ByteStringObject))
def raw(value):return value.original_bytes if isinstance(value,TextStringObject) else bytes(value)

def records(pdf):
    result=[]
    def walk(stream,resources,ctm,state,path):
        state=state.copy();stack=[];index=0;tm=identity;size=0; font=None
        for args,op in ContentStream(stream,pdf).operations:
            if op==b'q':stack.append((ctm,state.copy()))
            elif op==b'Q':ctm,state=stack.pop()
            elif op==b'cm':ctm=multiply(ctm,matrix(args))
            elif op in [b'Tc',b'Tw',b'Tz',b'Ts',b'TL',b'Tr']:state[op.decode()]=float(args[0])
            elif op==b'BT':tm=identity
            elif op==b'Tm':tm=matrix(args)
            elif op==b'Tf':font=resources['/Font'][args[0]].get_object();size=float(args[1])
            elif op in [b'Tj',b'TJ']:
                values=args if op==b'Tj' else args[0]
                if not any(is_string(v) for v in values):
                    dx=-sum(map(float,values))*size/1000*state['Tz']/100
                    tm=multiply(tm,matrix([1,0,0,1,dx,0]));continue
                width=2 if font['/Subtype']=='/Type0' else 1
                encoded=b'';slots=[];parts=[]
                for v in values:
                    if is_string(v):
                        data=raw(v);assert len(data)%width==0
                        encoded+=data;parts.append(data)
                    else:slots.append((len(encoded)//width,float(v)))
                result.append(dict(path=path+[index],matrix=multiply(ctm,tm),state=state.copy(),size=size,font=font,encoded=encoded,slots=slots,parts=parts,width=width))
                index+=1
            elif op==b'Do':
                form=resources['/XObject'][args[0]].get_object()
                walk(form,form.get('/Resources',resources),multiply(ctm,matrix(form.get('/Matrix',[1,0,0,1,0,0]))),state,path+[index]);index+=1
    page=pdf.pages[0]
    walk(page['/Contents'],page['/Resources'],identity,dict(Tc=0.,Tw=0.,Tz=100.,Ts=0.,TL=0.,Tr=0.),[])
    return result

def raster(page):
    pix=page.get_pixmap(matrix=fitz.Matrix(2,2),colorspace=fitz.csRGB,alpha=False)
    return Image.frombytes('RGB',(pix.width,pix.height),pix.samples)
def equal_page(a,b):
    assert a.rect==b.rect and a.cropbox==b.cropbox and a.rotation==b.rotation
    assert raster(a).tobytes()==raster(b).tobytes()
    assert a.get_text('rawdict')==b.get_text('rawdict')
def spans(page):return [(line,span) for block in page.get_text('rawdict')['blocks'] if 'lines' in block for line in block['lines'] for span in line['spans']]

checks=[]
outputs=sorted(root.glob('*-exported.pdf'));assert len(outputs)==12,len(outputs)
for output in outputs:
    prefix=output.name.removesuffix('-exported.pdf')
    original=root/f'{prefix}-original.pdf';reference=root/f'{prefix}-reference.pdf'
    with fitz.open(reference) as a,fitz.open(output) as b,fitz.open(original) as source:
        assert len(a)==len(b)==len(source)==2
        for x,y in zip(a,b):equal_page(x,y)
        equal_page(source[1],b[1])
    before,after,expected=map(records,map(PdfReader,[original,output,reference]))
    assert len(before)==len(after)==len(expected)==3
    for a,b,c in zip(before,after,expected):
        for key in ['path','matrix','state','size']:assert a[key]==b[key],(prefix,key)
        for key in ['encoded','slots','width']:assert b[key]==c[key],(prefix,key)
    for index in [0,2]:
        for key in ['encoded','slots','parts','width']:assert before[index][key]==after[index][key]
    assert [p.extract_text() for p in PdfReader(reference).pages]==[p.extract_text() for p in PdfReader(output).pages]
    checks.append(prefix)

font_bytes=Path('tests/fixtures/corpus/fonts/DejaVuSerif.ttf').read_bytes()
for kind in ['standard','simple','cid']:
    original=root/f'{kind}-substitute-original.pdf';output=root/f'{kind}-substitute-edited.pdf'
    original_pdf,output_pdf=PdfReader(original),PdfReader(output)
    before,after=records(original_pdf),records(output_pdf);assert len(before)==len(after)==3
    for a,b in zip(before,after):
        for key in ['path','matrix','state','size','slots']:assert a[key]==b[key],(kind,key)
    selected=after[1];assert selected['width']==2 and selected['encoded'].decode('utf-16-be')=='Cafè edit'
    assert [p.decode('utf-16-be') for p in selected['parts']]==['Ca','fè edi','t']
    font=selected['font']['/DescendantFonts'][0].get_object()
    assert font['/FontDescriptor']['/FontFile2'].get_data()==font_bytes
    widths=font['/W'];widths={int(widths[i]):float(widths[i+1][0]) for i in range(0,len(widths),2)}
    unit=float(output_pdf.pages[0].get('/UserUnit',1))
    with fitz.open(original) as a,fitz.open(output) as b:
        old,new=spans(a[0]),spans(b[0]);assert len(old)==len(new)==3
        assert old[0]==new[0] and old[2]==new[2]
        line,span=new[1];assert ''.join(g['c'] for g in span['chars'])=='Cafè edit'
        assert span['origin']==old[1][1]['origin'] and span['color']==old[1][1]['color'] and line['dir']==old[1][0]['dir']
        x,y=span['origin']
        for index,g in enumerate(span['chars']):
            assert abs(g['origin'][0]-x)<.003 and abs(g['origin'][1]-y)<.003,(kind,index,g,(x,y))
            advance=(math.floor(widths[ord(g['c'])]+.5)/1000*selected['size']+selected['state']['Tc'])
            advance-=sum(value for slot,value in selected['slots'] if slot==index+1)/1000*selected['size']
            advance*=selected['state']['Tz']/100*unit
            x+=selected['matrix'][0][0]*advance;y-=selected['matrix'][1][0]*advance
        diff=ImageChops.difference(raster(a[0]),raster(b[0]));assert diff.getbbox()
        # rawdict glyph boxes include UserUnit; this reader's rotation matrix
        # translation is still in raw page units (diagnosed at UserUnit 1.7).
        rotation=list(a[0].rotation_matrix)
        source_unit=float(original_pdf.pages[0].get('/UserUnit',1))
        rotation[4]*=source_unit;rotation[5]*=source_unit
        area=(fitz.Rect(old[1][1]['bbox'])|fitz.Rect(span['bbox']))*fitz.Matrix(*rotation)
        ImageDraw.Draw(diff).rectangle((area.x0*2-3,area.y0*2-3,area.x1*2+3,area.y1*2+3),fill=(0,0,0));assert diff.getbbox() is None
    checks.append(kind+' substitute logical slots, glyph advances and outside pixels')

report=dict(passed=True,readers=['MuPDF','pypdf'],referenceCases=len(outputs),substitutionCases=3,fontSha256=hashlib.sha256(font_bytes).hexdigest(),checks=checks)
(root/'independent-results.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report))
