"""Independent saved-output proof for reusable text font/line flow."""
from pathlib import Path
import hashlib,json,math,os
import fitz
from PIL import Image,ImageChops,ImageDraw
from pypdf import PdfReader
from pypdf.generic import ContentStream,TextStringObject,ByteStringObject

IDENTITY=(1.,0.,0.,1.,0.,0.)
def compose(a,b):
    aa,ab,ac,ad,ae,af=a;ba,bb,bc,bd,be,bf=b
    return (aa*ba+ac*bb,ab*ba+ad*bb,aa*bc+ac*bd,ab*bc+ad*bd,aa*be+ac*bf+ae,ab*be+ad*bf+af)
def raw(v):return v.original_bytes if isinstance(v,TextStringObject) else bytes(v)
def is_string(v):return isinstance(v,(TextStringObject,ByteStringObject))
def font_identity(font):
    descendant=font['/DescendantFonts'][0].get_object() if '/DescendantFonts' in font else font
    descriptor=descendant.get('/FontDescriptor')
    program=descriptor.get_object().get('/FontFile2') if descriptor else None
    return (str(font['/Subtype']),str(font.get('/BaseFont')),hashlib.sha256(program.get_object().get_data()).hexdigest() if program else None)
def records(reader,page_index=0):
    result=[]
    def walk(stream,resources,ctm,state,font,size,path):
        state=state.copy();stack=[];index=0;tm=line=IDENTITY
        for args,op in ContentStream(stream,reader).operations:
            if op==b'q':stack.append((ctm,state.copy(),font,size))
            elif op==b'Q':ctm,state,font,size=stack.pop()
            elif op==b'cm':ctm=compose(ctm,tuple(map(float,args)))
            elif op in [b'Tc',b'Tw',b'Tz',b'Ts',b'TL',b'Tr']:state[op.decode()]=float(args[0])
            elif op==b'Tf':font=resources['/Font'][args[0]].get_object();size=float(args[1])
            elif op==b'BT':tm=line=IDENTITY
            elif op==b'Tm':tm=line=tuple(map(float,args))
            elif op in [b'Td',b'TD',b'T*']:
                x,y=(0.,-state['TL']) if op==b'T*' else tuple(map(float,args))
                if op==b'TD':state['TL']=-y
                tm=line=compose(line,(1,0,0,1,x,y))
            elif op in [b'Tj',b'TJ',b"'",b'"']:
                if op==b'"':state['Tw'],state['Tc']=map(float,args[:2])
                if op in [b"'",b'"']:tm=line=compose(line,(1,0,0,1,0,-state['TL']))
                values=args[0] if op==b'TJ' else [args[-1]]
                if not any(is_string(v) for v in values):
                    if tm is not None:tm=compose(tm,(1,0,0,1,-sum(map(float,values))*size*state['Tz']/100000,0))
                    continue
                assert tm is not None,'Fixture unexpectedly depends on previous glyph advance'
                width=2 if font['/Subtype']=='/Type0' else 1
                data=b'';slots=[]
                for value in values:
                    if is_string(value):data+=raw(value)
                    else:slots.append((len(data)//width,float(value)))
                result.append(dict(path=path+[index],operator=op.decode(),matrix=compose(ctm,tm),state=state.copy(),size=size,font=font_identity(font),data=data,slots=slots))
                index+=1;tm=None
            elif op==b'Do':
                form=resources['/XObject'][args[0]].get_object()
                walk(form,form.get('/Resources',resources),compose(ctm,tuple(map(float,form.get('/Matrix',IDENTITY)))),state,font,size,path+[index]);index+=1
    page=reader.pages[page_index]
    walk(page['/Contents'],page['/Resources'],IDENTITY,dict(Tc=0.,Tw=0.,Tz=100.,Ts=0.,TL=0.,Tr=0.),None,0.,[])
    return result
def raster(page):
    pix=page.get_pixmap(matrix=fitz.Matrix(2,2),colorspace=fitz.csRGB,alpha=False)
    return Image.frombytes('RGB',(pix.width,pix.height),pix.samples)
def equal_page(a,b):
    assert a.rect==b.rect and a.cropbox==b.cropbox and a.rotation==b.rotation
    assert raster(a).tobytes()==raster(b).tobytes(),'Reference pixels differ'
    assert a.get_text('rawdict')==b.get_text('rawdict'),'Reference glyphs differ'
def spans(page):return [span for block in page.get_text('rawdict')['blocks'] if 'lines' in block for line in block['lines'] for span in line['spans']]
def verify(root):
    references=sorted(root.glob('*-exported.pdf'));substitutions=sorted(root.glob('*-substitute-edited.pdf'))
    assert len(references)==14,len(references)
    assert len(substitutions)==24,len(substitutions)
    checks=[]
    for output in references:
        prefix=output.name.removesuffix('-exported.pdf');reference=root/f'{prefix}-reference.pdf';original=root/f'{prefix}-original.pdf'
        with fitz.open(output) as b,fitz.open(reference) as a,fitz.open(original) as source:
            assert len(a)==len(b)==len(source)==2
            for x,y in zip(a,b):equal_page(x,y)
            equal_page(source[1],b[1])
        a,b=PdfReader(reference),PdfReader(output)
        assert records(a)==records(b),prefix
        assert [p.extract_text() for p in a.pages]==[p.extract_text() for p in b.pages]
        checks.append(prefix)
    for output in substitutions:
        prefix=output.name.removesuffix('-edited.pdf');original=root/f'{prefix}-original.pdf';line=int(prefix.split('-line-')[1][0]);selected=1+line
        a,b=PdfReader(original),PdfReader(output);before,after=records(a),records(b)
        assert len(before)==len(after)==5
        for i,(x,y) in enumerate(zip(before,after)):
            if i!=selected:assert x==y,(prefix,i,'unselected source record')
            else:
                for key in ['path','operator','matrix','state','size','slots']:assert x[key]==y[key],(prefix,key)
                assert y['data']=='Cafè edit'.encode('utf-16-be')
                assert y['font'][2],prefix
        with fitz.open(original) as source,fitz.open(output) as changed:
            old,new=spans(source[0]),spans(changed[0]);assert len(old)==len(new)==5
            for i,(x,y) in enumerate(zip(old,new)):
                if i!=selected:assert x==y,(prefix,i,'unselected glyphs')
            diff=ImageChops.difference(raster(source[0]),raster(changed[0]));mask=Image.new('L',diff.size,255);draw=ImageDraw.Draw(mask)
            for page,span,reader in [(source[0],old[selected],a),(changed[0],new[selected],b)]:
                rotation=fitz.Matrix(page.rotation_matrix);unit=float(reader.pages[0].get('/UserUnit',1));rotation.e*=unit;rotation.f*=unit
                box=fitz.Rect(span['bbox'])*rotation
                draw.rectangle((math.floor(box.x0*2)-3,math.floor(box.y0*2)-3,math.ceil(box.x1*2)+3,math.ceil(box.y1*2)+3),fill=0)
            assert Image.composite(diff,Image.new('RGB',diff.size),mask).getbbox() is None,(prefix,'outside pixels')
        checks.append(prefix)
    result={'passed':True,'readers':['MuPDF','pypdf'],'referenceCases':len(references),'substitutionCases':len(substitutions),'checks':checks}
    (root/'independent-results.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result))
if __name__=='__main__':verify(Path(os.environ.get('FOLIO_FORM_FLOW_ARTIFACTS','artifacts/form-text-flow-20261024/native')))
