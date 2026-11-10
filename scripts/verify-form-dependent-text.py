"""Check saved consecutive-piece edits with independent MuPDF and pypdf readers."""
import argparse
import hashlib
import json
import runpy
from pathlib import Path

import fitz
from PIL import Image, ImageChops
from pypdf import PdfReader
from pypdf.generic import ContentStream

helpers=runpy.run_path(str(Path(__file__).with_name('verify-form-text-flow.py')))
compose,raw,is_string,font_identity=(helpers[key] for key in ['compose','raw','is_string','font_identity'])
IDENTITY=(1.,0.,0.,1.,0.,0.)


def records(reader):
    """Evaluate authored horizontal advances from the actual PDF width tables."""
    result=[]
    def width_table(font):
        if font['/Subtype']=='/Type0':
            child=font['/DescendantFonts'][0].get_object();values=child['/W'];index=0;table={}
            while index<len(values):
                first=int(values[index]);following=values[index+1];index+=2
                if isinstance(following,list):
                    table.update({first+offset:float(value) for offset,value in enumerate(following)})
                else:
                    table.update({code:float(values[index]) for code in range(first,int(following)+1)});index+=1
            return table,2
        if '/Widths' in font:
            return {int(font['/FirstChar'])+i:float(value) for i,value in enumerate(font['/Widths'])},1
        assert font['/BaseFont']=='/Helvetica',font
        face=fitz.Font('helv')
        return {code:face.glyph_advance(code)*1000 for code in range(32,127)},1
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
                data=b''.join(raw(v) for v in values if is_string(v))
                table,length=width_table(font)
                if data:
                    result.append(dict(path=path+[index],matrix=compose(ctm,tm),state=state.copy(),size=size,
                                       font=font_identity(font),data=data,widths=table,codeLength=length))
                    index+=1
                shift=0.
                for value in values:
                    if is_string(value):
                        encoded=raw(value)
                        codes=[int.from_bytes(encoded[i:i+length],'big') for i in range(0,len(encoded),length)]
                        shift+=sum(table[code] for code in codes)*size/1000+len(codes)*state['Tc']
                        if length==1:shift+=codes.count(32)*state['Tw']
                    else:shift-=float(value)*size/1000
                tm=compose(tm,(1,0,0,1,shift*state['Tz']/100,0))
            elif op==b'Do':
                form=resources['/XObject'][args[0]].get_object()
                walk(form,form.get('/Resources',resources),compose(ctm,tuple(map(float,form.get('/Matrix',IDENTITY)))),state,font,size,path+[index]);index+=1
    page=reader.pages[0]
    walk(page['/Contents'],page['/Resources'],IDENTITY,dict(Tc=0.,Tw=0.,Tz=100.,Ts=0.,TL=0.,Tr=0.),None,0.,[])
    return result


def glyphs(page):
    return [(char['c'], tuple(char['origin']), tuple(char['bbox']), span['font'], span['size'], span['color'])
            for block in page.get_text('rawdict')['blocks'] if 'lines' in block
            for line in block['lines'] for span in line['spans'] for char in span['chars']
            if not char['c'].isspace()]


def bitmap(page):
    pix=page.get_pixmap(matrix=fitz.Matrix(2,2),alpha=False)
    return Image.frombytes('RGB',(pix.width,pix.height),pix.samples)


def near(a,b):
    # MuPDF quantizes these fractional PDF widths to integer font units. The
    # native tests use 0.001 pt; this independent check permits 0.01 pt but also
    # requires identical pixels everywhere outside the edited ink below.
    return len(a)==len(b) and all(abs(x-y)<0.01 for x,y in zip(a,b))


def verify(root, expected, first_replacement='Cafe'):
    outputs=sorted(root.glob('*-saved.pdf'))
    assert len(outputs)==expected, (len(outputs),expected)
    checks=[]
    for output in outputs:
        name=output.name.removesuffix('-saved.pdf')
        source=root/f'{name}-source.pdf'
        parts=name.split('-')
        selected=int(parts[1]) if parts[1].isdigit() else 0
        replacement=first_replacement if selected==0 else 'Cafeolder'
        with fitz.open(source) as original, fitz.open(output) as edited:
            assert len(original)==len(edited)==2
            old,new=glyphs(original[0]),glyphs(edited[0])
            assert ''.join(x[0] for x in old)=='Neighbor'+'Cafeold'*6,name
            start=8+selected*7
            assert ''.join(x[0] for x in new)=='Neighbor'+'Cafeold'*selected+replacement+'Cafeold'*(5-selected),name
            untouched_old=old[:start]+old[start+7:]
            untouched_new=new[:start]+new[start+len(replacement):]
            assert len(untouched_old)==len(untouched_new),name
            for a,b in zip(untouched_old,untouched_new):
                assert a[0]==b[0] and a[3:]==b[3:] and near(a[1],b[1]) and near(a[2],b[2]),(name,a,b)
            delta=max(abs(x-y) for a,b in zip(untouched_old,untouched_new) for x,y in zip(a[1]+a[2],b[1]+b[2]))
            assert ImageChops.difference(bitmap(original[1]),bitmap(edited[1])).getbbox() is None,name
            # MuPDF text boxes are unrotated; put the edited ink union into the
            # displayed coordinate system before checking changed raster pixels.
            area=fitz.Rect()
            for char in old[start:start+7]+new[start:start+len(replacement)]:
                area |= fitz.Rect(char[2])
            area=area*original[0].rotation_matrix
            difference=ImageChops.difference(bitmap(original[0]),bitmap(edited[0]))
            bounds=difference.getbbox()
            assert bounds is not None,name
            permitted=(int(area.x0*2)-3,int(area.y0*2)-3,int(area.x1*2)+4,int(area.y1*2)+4)
            assert bounds[0]>=permitted[0] and bounds[1]>=permitted[1] and bounds[2]<=permitted[2] and bounds[3]<=permitted[3],(name,bounds,permitted)
            old_records,new_records=records(PdfReader(source)),records(PdfReader(output))
            assert len(old_records)==len(new_records)==7,name
            for index,(a,b) in enumerate(zip(old_records,new_records)):
                assert all(abs(x-y)<0.0001 for x,y in zip(a['matrix'],b['matrix'])),(name,'PDF advance',a,b)
                for key in ['path','state','size','font','widths','codeLength']:
                    assert a[key]==b[key],(name,key)
                if index!=selected+1:assert a['data']==b['data'],name
            # pypdf's high-level extractor omits inherited-resource form text;
            # inspect the parsed encoded shows and widths directly instead.
            text=''.join(record['data'].decode('utf-16-be' if record['codeLength']==2 else 'ascii') for record in new_records)
            compact=''.join(text.split())
            assert compact=='Neighbor'+'Cafeold'*selected+replacement+'Cafeold'*(5-selected),(name,compact)
            checks.append(dict(name=name,unselectedGlyphs=len(untouched_old),difference=bounds,maxIndependentGlyphDelta=delta,
                               sourceSha256=hashlib.sha256(source.read_bytes()).hexdigest(),
                               outputSha256=hashlib.sha256(output.read_bytes()).hexdigest()))
    return dict(passed=True,cases=checks,mupdfVersion=fitz.VersionBind)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root',type=Path)
    parser.add_argument('--expected',type=int,required=True)
    parser.add_argument('--first-replacement',default='Cafe')
    args=parser.parse_args()
    result=verify(args.root,args.expected,args.first_replacement)
    output=args.root/'independent-results.json'
    with output.open('x',encoding='utf-8') as stream:
        json.dump(result,stream,indent=2)
        stream.write('\n')
    print(f"{len(result['cases'])} independent saved-edit checks passed")
