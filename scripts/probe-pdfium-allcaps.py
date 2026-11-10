"""Verify PDFium's existing AllCap glyph fallback keeps coherent advances."""
import argparse,hashlib,json,runpy
from pathlib import Path
from PIL import ImageChops
from pypdf import PdfWriter
from pypdf.generic import DictionaryObject,NameObject,ArrayObject,FloatObject,NumberObject,DecodedStreamObject

helpers=runpy.run_path(str(Path(__file__).with_name('probe-pdfium-form-inheritance.py')))

def fixture(path,width,explicit):
    writer=PdfWriter();page=writer.add_blank_page(width=320,height=240)
    descriptor=DictionaryObject({NameObject('/Type'):NameObject('/FontDescriptor'),NameObject('/FontName'):NameObject('/Arial'),
        NameObject('/Flags'):NumberObject(65568),NameObject('/FontBBox'):ArrayObject(map(NumberObject,[-100,-300,1200,1000])),
        NameObject('/ItalicAngle'):NumberObject(0),NameObject('/Ascent'):NumberObject(900),NameObject('/Descent'):NumberObject(-300),
        NameObject('/CapHeight'):NumberObject(700),NameObject('/StemV'):NumberObject(80)})
    widths=[FloatObject(width if code==65 else 300) for code in range(32,123)]
    font=DictionaryObject({NameObject('/Type'):NameObject('/Font'),NameObject('/Subtype'):NameObject('/TrueType'),
        NameObject('/BaseFont'):NameObject('/Arial'),NameObject('/Encoding'):NameObject('/WinAnsiEncoding'),
        NameObject('/FirstChar'):NumberObject(32),NameObject('/LastChar'):NumberObject(122),
        NameObject('/Widths'):ArrayObject(widths),NameObject('/FontDescriptor'):writer._add_object(descriptor)})
    page[NameObject('/Resources')]=DictionaryObject({NameObject('/Font'):DictionaryObject({NameObject('/F'):writer._add_object(font)})})
    pieces=['BT /F 16 Tf 1 0 0 1 50 150 Tm']
    for index,char in enumerate('aaAA'):
        if explicit:pieces.append(f'1 0 0 1 {50+index*width*16/1000:.8f} 150 Tm')
        pieces.append(f'({char}) Tj')
    pieces.append('ET');stream=DecodedStreamObject();stream.set_data(' '.join(pieces).encode())
    page[NameObject('/Contents')]=writer._add_object(stream)
    with path.open('xb') as target:writer.write(target)

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--dll',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=False)
    engine=helpers['Pdfium'](args.dll);cases=[]
    try:
        for label,width in [('integer',600.),('fractional',600.25)]:
            bitmaps=[];positions=[]
            for explicit in [False,True]:
                stem=f'{label}-'+('explicit' if explicit else 'implicit');path=args.output/f'{stem}.pdf';fixture(path,width,explicit)
                bitmap,characters=engine.render(path);assert helpers['has_ink'](bitmap)
                characters=[c for c in characters if c['unicode'] not in [10,13,32]]
                bitmaps.append(bitmap);positions.append(characters);bitmap.save(args.output/f'{stem}.png')
            counts=[len(value) for value in positions]
            delta=max(abs(a-b) for p,q in zip(*positions) for a,b in zip(p['origin'],q['origin'])) if counts==[4,4] else None
            difference=ImageChops.difference(*bitmaps).getbbox()
            cases.append(dict(name=label,characterCounts=counts,maxOriginDelta=delta,difference=difference,passed=delta is not None and delta<0.001 and difference is None))
            print(f'{label}: characters={counts}, delta={delta}, difference={difference}',flush=True)
    finally:engine.close()
    result=dict(passed=all(c['passed'] for c in cases),cases=cases,dllSha256=hashlib.sha256(args.dll.read_bytes()).hexdigest(),scriptSha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),limits=['Native fallback compatibility; not a cross-reader AllCap policy claim.'])
    (args.output/'results.json').write_text(json.dumps(result,indent=2)+'\n')
    if not result['passed']:raise SystemExit(1)

if __name__=='__main__':main()
