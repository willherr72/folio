"""Independent font-program, glyph-mapping, geometry and shared-copy checks."""
import hashlib,json,os,math
from pathlib import Path
import fitz
from pypdf import PdfReader
from fontTools.ttLib import TTFont
from PIL import Image,ImageChops,ImageDraw

root=Path(os.environ.get('FOLIO_FORM_FONT_ARTIFACTS','artifacts/form-font-fixtures'))
font_path=Path('tests/fixtures/corpus/fonts/DejaVuSerif.ttf')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
font_hash=sha(font_path);font=TTFont(font_path);cmap=font.getBestCmap();units=font['head'].unitsPerEm
def image(page):
    pix=page.get_pixmap(matrix=fitz.Matrix(2,2),colorspace=fitz.csRGB,alpha=False)
    return Image.frombytes('RGB',(pix.width,pix.height),pix.samples)
def spans(page):return [span for block in page.get_text('rawdict')['blocks'] if 'lines' in block for line in block['lines'] for span in line['spans']]
def text(span):return ''.join(c['c'] for c in span['chars'])
def programs(doc):return {hashlib.sha256(doc.extract_font(f[0])[3]).hexdigest() for page in doc for f in page.get_fonts(full=True) if doc.extract_font(f[0])[3]}
def equal_page(a,b):
    assert a.rect==b.rect and a.rotation==b.rotation
    assert image(a).tobytes()==image(b).tobytes()
    assert a.get_text('rawdict')==b.get_text('rawdict')
def fonts(resources):
    resources=resources.get_object();yield from (f.get_object() for f in resources.get('/Font',{}).get_object().values()) if '/Font' in resources else []
    for value in resources.get('/XObject',{}).get_object().values() if '/XObject' in resources else []:
        stream=value.get_object()
        if '/Resources' in stream:yield from fonts(stream['/Resources'])
def verify(original,edited,expected='Cafè edit',duplicates=True):
    with fitz.open(original) as before,fitz.open(edited) as after:
        assert len(before)==2 and len(after)==(3 if duplicates else 2)
        equal_page(before[1],after[1])
        if duplicates:equal_page(before[0],after[2])
        old_spans,new_spans=spans(before[0]),spans(after[0]);assert len(old_spans)==len(new_spans)==3
        assert old_spans[0]==new_spans[0] and old_spans[2]==new_spans[2]
        old,new=old_spans[1],new_spans[1];assert text(new)==expected
        assert new['size']==old['size'] and new['color']==old['color'] and new['origin']==old['origin']
        assert programs(after)==programs(before)|{font_hash}
        x,y=old['origin']
        for glyph,c in zip(new['chars'],expected):
            assert glyph['c']==c and abs(glyph['origin'][0]-x)<0.001 and abs(glyph['origin'][1]-y)<0.001
            # MuPDF's extracted positions use integer 1/1000-em advances even
            # when the PDF /W table contains fractional values. Verify those
            # exact stored widths separately below, then model reader rounding.
            x+=math.floor(font['hmtx'][cmap[ord(c)]][0]/units*1000+0.5)/1000*old['size']
        diff=ImageChops.difference(image(before[0]),image(after[0]));assert diff.getbbox()
        area=(fitz.Rect(old['bbox'])|fitz.Rect(new['bbox']))*before[0].rotation_matrix
        ImageDraw.Draw(diff).rectangle((area.x0*2-3,area.y0*2-3,area.x1*2+3,area.y1*2+3),fill=(0,0,0));assert diff.getbbox() is None
    pdf=PdfReader(edited);original_pdf=PdfReader(original)
    # pypdf omits text from the original inherited-resource leaf in this fixture.
    # MuPDF above verifies every original glyph; pypdf independently verifies
    # the new explicit resource and unchanged extraction elsewhere.
    extracted=pdf.pages[0].extract_text();baseline=original_pdf.pages[0].extract_text()
    assert extracted.count(expected)==1
    if 'Café old' in baseline:
        assert extracted.replace(expected,'Café old')==baseline
    else:
        assert extracted.replace(expected,'').strip()==baseline.strip()
    assert pdf.pages[1].extract_text()==original_pdf.pages[1].extract_text()
    selected=[f for f in fonts(pdf.pages[0]['/Resources']) if str(f.get('/BaseFont'))=='/Folio'+font_hash];assert selected
    for resource in selected:
        descendant=resource['/DescendantFonts'][0].get_object();mapping=descendant['/CIDToGIDMap'].get_data()
        assert hashlib.sha256(descendant['/FontDescriptor']['/FontFile2'].get_data()).hexdigest()==font_hash
        widths=descendant['/W'];widths={int(widths[i]):float(widths[i+1][0]) for i in range(0,len(widths),2)}
        for c in expected:
            assert int.from_bytes(mapping[ord(c)*2:ord(c)*2+2],'big')==font.getGlyphID(cmap[ord(c)])
            assert abs(widths[ord(c)]-font['hmtx'][cmap[ord(c)]][0]/units*1000)<0.001

checks=[]
for kind in ['simple','cid']:
    for rotation in [0,90,180,270]:
        prefix=f'{kind}-{rotation}';verify(root/f'{prefix}-original.pdf',root/f'{prefix}-edited.pdf');checks.append(prefix)
desktop=os.environ.get('FOLIO_FORM_FONT_DESKTOP_ROOT')
if desktop:
    desktop=Path(desktop);result=json.loads((desktop/'results.json').read_text());assert result['passed']
    for kind in ['simple','cid']:
        original=desktop/f'{kind}-original.pdf';assert sha(original)==result['sources'][kind]
        verify(original,desktop/f'{kind}-substituted.pdf',duplicates=False)
        verify(original,desktop/f'{kind}-reedited.pdf',expected='Cafè again',duplicates=False)
        with fitz.open(original) as before,fitz.open(desktop/f'{kind}-undo.pdf') as undo:
            for a,b in zip(before,undo):equal_page(a,b)
        checks.append(f'{kind} Windows substitution/re-edit/undo')
report={'passed':True,'readers':['MuPDF','pypdf','fontTools'],'fullFontSha256':font_hash,'checks':checks,'verified':['chosen embedded program and Unicode-to-glyph IDs','font-derived character advances and original baseline/size/color','unchanged other copies and pages','unchanged outside-area pixels']}
((desktop or root)/'independent-results.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report))
