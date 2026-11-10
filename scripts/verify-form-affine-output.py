"""Independent reference raster/glyph and affine font substitution checks."""
import hashlib,json,math,os
from pathlib import Path
import fitz
from pypdf import PdfReader
from pypdf.generic import ContentStream
from fontTools.ttLib import TTFont
from PIL import Image,ImageChops,ImageDraw
root=Path(os.environ.get('FOLIO_FORM_AFFINE_ARTIFACTS','artifacts/form-affine-fixtures'))
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def raster(page):
    pix=page.get_pixmap(matrix=fitz.Matrix(2,2),colorspace=fitz.csRGB,alpha=False)
    return Image.frombytes('RGB',(pix.width,pix.height),pix.samples)
def equal_page(a,b):
    assert a.rect==b.rect and a.rotation==b.rotation and a.cropbox==b.cropbox
    assert raster(a).tobytes()==raster(b).tobytes()
    assert a.get_text('rawdict')==b.get_text('rawdict')
def programs(doc):return {hashlib.sha256(doc.extract_font(f[0])[3]).hexdigest() for page in doc for f in page.get_fonts(full=True) if doc.extract_font(f[0])[3]}
def equal(reference,exported):
    with fitz.open(reference) as a,fitz.open(exported) as b:
        assert len(a)==len(b)==2
        for x,y in zip(a,b):equal_page(x,y)
        assert programs(a)==programs(b)
    assert [p.extract_text() for p in PdfReader(reference).pages]==[p.extract_text() for p in PdfReader(exported).pages]

# Independently interpret only the authored fixture operators, with generic 3x3
# matrix multiplication. This proves the source/output matrix and font identity;
# it is not a second editor or a production admission parser.
identity=[[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]]
def matrix(values):
    a,b,c,d,e,f=map(float,values);return [[a,c,e],[b,d,f],[0.,0.,1.]]
def multiply(a,b):return [[sum(a[i][k]*b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]
def records(pdf):
    out=[]
    def walk(stream,resources,ctm,path):
        index=0;stack=[];font=None;size=None;tm=identity
        for args,op in ContentStream(stream,pdf).operations:
            if op==b'q':stack.append(ctm)
            elif op==b'Q':ctm=stack.pop()
            elif op==b'cm':ctm=multiply(ctm,matrix(args))
            elif op==b'BT':tm=identity;font=None;size=None
            elif op==b'Tm':tm=matrix(args)
            elif op==b'Tf':font=resources['/Font'][args[0]].get_object();size=float(args[1])
            elif op==b'Tj':out.append({'path':path+[index],'matrix':multiply(ctm,tm),'size':size,'font':font});index+=1
            elif op==b'Do':
                form=resources['/XObject'][args[0]].get_object();transform=multiply(ctm,matrix(form.get('/Matrix',[1,0,0,1,0,0])))
                walk(form,form.get('/Resources',resources),transform,path+[index]);index+=1
    page=pdf.pages[0];walk(page['/Contents'],page['/Resources'],identity,[]);return out
def spans(page):return [(line,span) for block in page.get_text('rawdict')['blocks'] if 'lines' in block for line in block['lines'] for span in line['spans']]
font_path=Path('tests/fixtures/corpus/fonts/DejaVuSerif.ttf');font_hash=sha(font_path);font=TTFont(font_path);cmap=font.getBestCmap();units=font['head'].unitsPerEm
def substitute(original,exported,expected='Cafè edit'):
    before_pdf,after_pdf=PdfReader(original),PdfReader(exported);before,after=records(before_pdf),records(after_pdf)
    assert len(before)==len(after)==3
    for a,b in zip(before,after):assert a['path']==b['path'] and a['matrix']==b['matrix'] and a['size']==b['size']
    selected=after[1];resource=selected['font'];assert str(resource['/BaseFont'])=='/Folio'+font_hash
    descendant=resource['/DescendantFonts'][0].get_object();assert hashlib.sha256(descendant['/FontDescriptor']['/FontFile2'].get_data()).hexdigest()==font_hash
    mapping=descendant['/CIDToGIDMap'].get_data();widths=descendant['/W'];widths={int(widths[i]):float(widths[i+1][0]) for i in range(0,len(widths),2)}
    for c in expected:
        assert int.from_bytes(mapping[ord(c)*2:ord(c)*2+2],'big')==font.getGlyphID(cmap[ord(c)])
        assert abs(widths[ord(c)]-font['hmtx'][cmap[ord(c)]][0]/units*1000)<0.001
    with fitz.open(original) as a,fitz.open(exported) as b:
        assert len(a)==len(b)==2;equal_page(a[1],b[1]);old,new=spans(a[0]),spans(b[0]);assert len(old)==len(new)==3
        assert old[0]==new[0] and old[2]==new[2]
        (old_line,old_span),(line,span)=old[1],new[1];assert ''.join(g['c'] for g in span['chars'])==expected
        assert old_line['dir']==line['dir'] and old_span['color']==span['color'] and old_span['size']==span['size'] and old_span['origin']==span['origin']
        x,y=old_span['origin'];m=selected['matrix'];size=selected['size']
        for g,c in zip(span['chars'],expected):
            assert abs(g['origin'][0]-x)<0.002 and abs(g['origin'][1]-y)<0.002,(exported,c,g['origin'],(x,y))
            advance=math.floor(widths[ord(c)]+0.5)/1000*size
            x+=m[0][0]*advance;y-=m[1][0]*advance
        assert programs(b)==programs(a)|{font_hash}
        diff=ImageChops.difference(raster(a[0]),raster(b[0]));assert diff.getbbox()
        area=(fitz.Rect(old_span['bbox'])|fitz.Rect(span['bbox']))*a[0].rotation_matrix
        ImageDraw.Draw(diff).rectangle((area.x0*2-3,area.y0*2-3,area.x1*2+3,area.y1*2+3),fill=(0,0,0));assert diff.getbbox() is None
    assert after_pdf.pages[0].extract_text().count(expected)==1

checks=[]
outputs=sorted(root.glob('*-exported.pdf'));assert len(outputs)==92,len(outputs)
for output in outputs:
    prefix=output.name.removesuffix('-exported.pdf');equal(root/f'{prefix}-reference.pdf',output)
    with fitz.open(root/f'{prefix}-original.pdf') as a,fitz.open(output) as b:equal_page(a[1],b[1])
    checks.append(prefix)
substitutes=sorted(root.glob('*-substitute.pdf'));assert len(substitutes)==8,len(substitutes)
for output in substitutes:
    prefix=output.name.removesuffix('-substitute.pdf');substitute(root/f'{prefix}-original.pdf',output);checks.append(prefix+' substitute')
desktop=os.environ.get('FOLIO_FORM_AFFINE_DESKTOP_ROOT')
if desktop:
    desktop=Path(desktop);result=json.loads((desktop/'results.json').read_text());assert result['passed']
    for prefix in ['form-rotation-0','mirror-0','text-rotation-270','composed-90']:
        original=desktop/f'{prefix}-original.pdf';assert sha(original)==result['sources'][prefix]
        equal(root/f'{prefix}-longer-reference.pdf',desktop/f'{prefix}-edited.pdf')
        for suffix in ['undo','restored']:equal(original,desktop/f'{prefix}-{suffix}.pdf')
        checks.append(prefix+' Windows edit/undo/restore')
    prefix='cid-form-shear-90';original=desktop/f'{prefix}-original.pdf';assert sha(original)==result['sources'][prefix]
    equal(original,desktop/f'{prefix}-undo.pdf');substitute(original,desktop/f'{prefix}-edited.pdf');substitute(original,desktop/f'{prefix}-reedited.pdf','Cafè again')
    checks.append(prefix+' Windows substitution/re-edit/undo')
report={'passed':True,'readers':['MuPDF','pypdf','fontTools'],'referenceCases':len(outputs),'substituteCases':len(substitutes),'fullFontSha256':font_hash,'checks':checks,'verified':['exact independently authored reference pixels and glyph geometry','unchanged second pages and original font programs','unchanged affine matrices and chosen full program/glyph IDs/widths','substituted character positions from original affine matrix','unchanged other occurrences and outside-area pixels']}
((desktop or root)/'independent-results.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
