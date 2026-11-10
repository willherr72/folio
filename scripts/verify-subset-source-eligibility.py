"""Independent text, rendered-page and embedded-font proof for subset edits."""
import argparse, hashlib, json
from io import BytesIO
from pathlib import Path
import fitz
from pypdf import PdfReader, PdfWriter
from pypdf.generic import NameObject, DecodedStreamObject, ContentStream

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--corpus',type=Path,required=True)
p.add_argument('--out',type=Path,required=True)
p.add_argument('--routes',nargs='+',choices=['legacy','complete'],default=['legacy','complete'])
p.add_argument('--substitutions',action='store_true')
args=p.parse_args();args.out.mkdir(exist_ok=False)

def fonts(path):
    values=[]
    for font in PdfReader(path).pages[0]['/Resources']['/Font'].values():
        font=font.get_object()
        if '/DescendantFonts' in font: font=font['/DescendantFonts'][0].get_object()
        if '/FontDescriptor' in font and '/FontFile2' in font['/FontDescriptor']:
            values.append(hashlib.sha256(font['/FontDescriptor']['/FontFile2'].get_data()).hexdigest())
    return sorted(set(values))

results=[]
def compare(actual,expected):
    with fitz.open(actual) as a,fitz.open(expected) as b:
        assert len(a)==len(b)==1
        assert a[0].get_text()==b[0].get_text(),(actual,'text')
        assert a[0].rect==b[0].rect,(actual,'displayed frame')
        for scale in [1,2]:
            x=a[0].get_pixmap(matrix=fitz.Matrix(scale,scale));y=b[0].get_pixmap(matrix=fitz.Matrix(scale,scale))
            assert (x.width,x.height,x.samples)==(y.width,y.height,y.samples),(actual,scale,'pixels')

if args.substitutions:
    from reportlab.pdfgen import canvas
    from reportlab.pdfbase import pdfmetrics
    from reportlab.pdfbase.ttfonts import TTFont
    font_path=Path('tests/fixtures/corpus/fonts/DejaVuSerif.ttf')
    font_hash=hashlib.sha256(font_path.read_bytes()).hexdigest()
    pdfmetrics.registerFont(TTFont('IndependentSubstitute',str(font_path)))

for rotation in [0,90,180,270]:
    source=args.corpus/f'source-{rotation}.pdf'
    writer=PdfWriter(clone_from=source);page=writer.pages[0];data=page.get_contents().get_data()
    assert data.count(b'<004F006C0064>')==1
    stream=DecodedStreamObject();stream.set_data(data.replace(b'<004F006C0064>',b'<0045006400690074>',1))
    page[NameObject('/Contents')]=writer._add_object(stream)
    reference=args.out/f'edited-reference-{rotation}.pdf';writer.write(reference)
    for route in args.routes:
        for action,expected in [('saved',reference),('restored',source)]:
            actual=args.corpus/f'{action}-{rotation}-{route}.pdf'
            compare(actual,expected)
            assert fonts(actual)==fonts(source),(actual,'font programs')
            results.append(dict(file=actual.name,sha256=hashlib.sha256(actual.read_bytes()).hexdigest(),exactText=True,exactPixels=[1,2],fontPrograms=fonts(actual)))
    if args.substitutions:
        buffer=BytesIO();c=canvas.Canvas(buffer,pagesize=(300,400),pageCompression=0)
        c.setFont('IndependentSubstitute',16);c.setFillColorRGB(.2,.4,.6);c.drawString(30,300,'Edit ')
        c.setFont('Helvetica',16);c.setFillColorRGB(0,0,0);c.drawString(150,300,'Neighbor');c.save()
        ref_writer=PdfWriter(clone_from=BytesIO(buffer.getvalue()));ref_writer.pages[0].rotate(rotation)
        substituted_reference=args.out/f'substituted-reference-{rotation}.pdf';ref_writer.write(substituted_reference)
        for route in args.routes:
            actual=args.corpus/f'substituted-{rotation}-{route}.pdf';compare(actual,substituted_reference)
            reader=PdfReader(actual);page=reader.pages[0];active=None;first=None
            for operands,op in ContentStream(page.get_contents(),reader).operations:
                if op==b'Tf': active=operands[0]
                if op in [b'Tj',b'TJ']:
                    pieces=operands[0] if op==b'TJ' else operands
                    encoded=b''.join(piece.original_bytes for piece in pieces if isinstance(piece,(str,bytes)))
                    if encoded: first=(active,encoded);break
            assert first and first[1]=='Edit '.encode('utf-16-be'),(actual,'actual trailing source space')
            font=page['/Resources']['/Font'][first[0]]['/DescendantFonts'][0].get_object()
            selected=font['/FontDescriptor']['/FontFile2'].get_data()
            assert hashlib.sha256(selected).hexdigest()==font_hash,(actual,'selected font bytes')
            results.append(dict(file=actual.name,sha256=hashlib.sha256(actual.read_bytes()).hexdigest(),exactText=True,exactPixels=[1,2],selectedFont=font_hash,actualTrailingSpace=True))
report=dict(passed=True,documents=len(results),results=results)
(args.out/'result.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps(dict(passed=True,documents=len(results),exactPixels=[1,2],fontPrograms=True)))
