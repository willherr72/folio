"""Compare implicit font advances with explicit PDF-width positions, without Folio."""
import argparse
import hashlib
import json
from pathlib import Path
import runpy
from PIL import ImageChops
from pypdf import PdfReader, PdfWriter
from pypdf.generic import DictionaryObject, NameObject, ArrayObject, FloatObject, DecodedStreamObject

helpers=runpy.run_path(str(Path(__file__).with_name('probe-pdfium-form-inheritance.py')))
digest=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()

def source_font(page):
    def visit(stream,resources,depth):
        assert depth<12
        resources=stream.get('/Resources',resources).get_object()
        if b'Tj' in stream.get_data():
            return resources['/Font']['/F1'].get_object()
        for obj in resources.get('/XObject',{}).values():
            found=visit(obj.get_object(),resources,depth+1)
            if found is not None:return found
    for obj in page['/Resources']['/XObject'].values():
        found=visit(obj.get_object(),page['/Resources'],0)
        if found is not None:return found
    raise ValueError('Fixture has no source form font')

def widths(font,integral):
    table={}
    if font['/Subtype']=='/Type0':
        child=font['/DescendantFonts'][0].get_object()
        values=child['/W'];index=0
        while index<len(values):
            first=int(values[index]);value=values[index+1];index+=2
            if isinstance(value,ArrayObject):
                for offset,width in enumerate(value):
                    number=round(float(width)) if integral else float(width)
                    value[offset]=FloatObject(number);table[first+offset]=number
            else:
                number=round(float(values[index])) if integral else float(values[index])
                values[index]=FloatObject(number);index+=1
                table.update({code:number for code in range(first,int(value)+1)})
        return table
    first=int(font['/FirstChar'])
    for offset,width in enumerate(font['/Widths']):
        number=round(float(width)) if integral else float(width)
        font['/Widths'][offset]=FloatObject(number);table[first+offset]=number
    return table

def fixture(input_path,output,integral,explicit,mode='array'):
    reader=PdfReader(input_path);writer=PdfWriter()
    font=source_font(reader.pages[0]).clone(writer)
    table=widths(font,integral)
    if mode!='array':
        width=500.0 if integral else 500.25
        table={code:width for code in range(256)}
        if mode=='range':
            font['/DescendantFonts'][0].get_object()[NameObject('/W')]=ArrayObject([
                FloatObject(0),FloatObject(255),FloatObject(width)])
        elif mode=='default':
            child=font['/DescendantFonts'][0].get_object()
            child[NameObject('/W')]=ArrayObject()
            child[NameObject('/DW')]=FloatObject(width)
        elif mode=='missing':
            font[NameObject('/FirstChar')]=FloatObject(32)
            font[NameObject('/LastChar')]=FloatObject(32)
            font[NameObject('/Widths')]=ArrayObject([FloatObject(width)])
            font['/FontDescriptor'].get_object()[NameObject('/MissingWidth')]=FloatObject(width)
        else:
            raise ValueError(mode)
    first,second='Cafe old','Cafe'
    x=50+sum(table[ord(char)] for char in first)*16/1000
    encode=lambda text: text.encode('utf-16-be' if font['/Subtype']=='/Type0' else 'ascii').hex()
    position=f'1 0 0 1 {x:.8f} 150 Tm ' if explicit else ''
    content=f'BT /F 16 Tf 1 0 0 1 50 150 Tm <{encode(first)}> Tj {position}<{encode(second)}> Tj ET'
    page=writer.add_blank_page(width=320,height=240)
    page[NameObject('/Resources')]=DictionaryObject({NameObject('/Font'):DictionaryObject({NameObject('/F'):font.indirect_reference})})
    stream=DecodedStreamObject();stream.set_data(content.encode('ascii'))
    page[NameObject('/Contents')]=writer._add_object(stream)
    with output.open('xb') as target:writer.write(target)
    return x

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixtures',type=Path,default=Path(__file__).resolve().parent.parent/'tests/fixtures/pdfium-font-advances')
    parser.add_argument('--dll',type=Path,default=Path('src-tauri/resources/pdfium/pdfium.dll'))
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--expect-known-defects',action='store_true')
    parser.add_argument('--extended',action='store_true',help='Also verify range, default and missing widths')
    args=parser.parse_args()
    args.output.mkdir(parents=True,exist_ok=False)
    engine=helpers['Pdfium'](args.dll)
    report=dict(dllSha256=digest(args.dll),scriptSha256=digest(Path(__file__)),diagnostic=args.expect_known_defects,cases=[])
    try:
        configurations=[('simple','array'),('cid','array')]
        if args.extended:configurations += [('simple','missing'),('cid','range'),('cid','default')]
        for kind,mode in configurations:
            for integral in [False,True]:
                label=f'{kind}-'+('integer' if integral else 'fractional')
                if mode!='array':label+=f'-{mode}'
                bitmaps=[];independent=[];origins=[];hashes={}
                for explicit in [False,True]:
                    stem=label+('-explicit' if explicit else '-implicit')
                    path=args.output/f'{stem}.pdf'
                    expected=fixture(args.fixtures/f'{kind}-leading-source.pdf',path,integral,explicit,mode)
                    native,characters=engine.render(path);other=helpers['mupdf'](path)
                    assert helpers['has_ink'](native) and helpers['has_ink'](other)
                    assert ''.join(chr(c['unicode']) for c in characters if c['unicode'] not in [10,13])=='Cafe oldCafe'
                    native.save(args.output/f'{stem}-pdfium.png');other.save(args.output/f'{stem}-mupdf.png')
                    bitmaps.append(native);independent.append(other);origins.append(characters);hashes[path.name]=digest(path)
                assert len(origins[0])==len(origins[1])
                delta=max(abs(a-b) for p,q in zip(origins[0],origins[1]) for a,b in zip(p['origin'],q['origin']))
                native_box=ImageChops.difference(*bitmaps).getbbox()
                independent_box=ImageChops.difference(*independent).getbbox()
                equal=delta<0.001 and native_box is None
                correct=(not equal if args.expect_known_defects and not integral else equal) and independent_box is None
                report['cases'].append(dict(name=label,expectedSecondX=expected,maxNativeOriginDelta=delta,
                    nativeDifference=native_box,independentDifference=independent_box,passed=correct,pdfSha256=hashes))
                print(f'{label}: native origin delta={delta:.6f}, native pixels={native_box}, independent pixels={independent_box}',flush=True)
    finally:engine.close()
    report['passed']=all(case['passed'] for case in report['cases']) and len(report['cases'])==len(configurations)*2
    (args.output/'results.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    if not report['passed']:raise SystemExit('Font-advance probe failed')

if __name__=='__main__':main()
