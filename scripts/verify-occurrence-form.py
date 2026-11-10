"""Independent complete-source reusable form editing oracle.

Selected reference is authored independently by changing only the first nested
leaf text in the generated PDF. Published forms must remain reusable.
"""
import argparse,hashlib,json
from pathlib import Path
import fitz
from pypdf import PdfReader
from pypdf.generic import ContentStream

def nested_shows(stream,resources,pdf,depth=0):
    assert depth<=12,'unexpected nested form depth'
    result=[]
    for operands,operator in ContentStream(stream,pdf).operations:
        if operator==b'Do':
            child=resources['/XObject'][operands[0]].get_object()
            assert child['/Subtype']=='/Form'
            result.extend(nested_shows(child,child.get('/Resources',resources),pdf,depth+1))
        elif operator==b'Tj':result.append((depth,str(operands[0])))
    return result

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--corpus',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args();records=[];filled_pages=0
for path in sorted(args.corpus.glob('form-*-result.pdf')):
    label=path.name.removesuffix('-result.pdf')
    original=args.corpus/f'{label}-source.pdf';reference=args.corpus/f'{label}-reference.pdf'
    inputs=[path,original,reference];hashes={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs}
    order=[0,1] if '-unique-' in label else [0,1,0]
    selected=0 if '-unique-' in label or '-first-' in label else 2
    before=PdfReader(original);actual=PdfReader(path)
    assert len(actual.pages)==len(order),label
    fields=actual.trailer['/Root']['/AcroForm']['/Fields']
    assert len(fields)==1 and len(fields[0].get_object()['/Kids'])==len(order)
    assert actual.get_fields()['shared']['/V']=='Keep field'
    ids=[p.indirect_reference for p in actual.pages]
    assert actual.trailer['/Root']['/Outlines']['/First']['/Dest'][0]==ids[0]
    for index,old in enumerate(order):
        page=actual.pages[index];source=before.pages[old]
        shows=nested_shows(page.get_contents(),page['/Resources'],actual)
        expected=['Old','Old'] if old==0 else ['Old']
        if index==selected and label.endswith('-edit'):expected[0]='New'
        assert [text for depth,text in shows]==expected,(label,index,'nested form strings')
        assert all(depth==2 for depth,text in shows),(label,index,'retained nested form graph')
        assert page.mediabox==source.mediabox and page.cropbox==source.cropbox and page.rotation==source.rotation
        assert float(page.get('/UserUnit',1))==float(source.get('/UserUnit',1))
        links={};widgets=[];notes=[]
        for ref in page['/Annots']:
            a=ref.get_object()
            if a['/Subtype']=='/Widget':widgets.append(a);assert a.raw_get('/Parent')==fields[0]
            elif a['/Subtype']=='/Text':notes.append(a);assert a['/Contents']=='Keep review'
            elif a['/Subtype']=='/Link':
                name=str(a['/NM']);dest=a['/A']['/D'] if name=='named' else a['/Dest']
                target=actual.get_destination_page_number(actual.named_destinations[dest]) if isinstance(dest,str) else ids.index(dest[0])
                assert target==(index if name=='self' else order.index(1-old)),(label,index,name,target)
                links[name]=target
        assert len(widgets)==len(notes)==1 and set(links)=={'direct','named','self'}
    with fitz.open(original) as old,fitz.open(path) as changed,fitz.open(reference) as expected:
        for index,old_index in enumerate(order):
            wanted=expected[0] if index==selected else old[old_index]
            assert changed[index].get_text()==wanted.get_text(),(label,index,'text')
            for scale in [1,2]:
                a=changed[index].get_pixmap(matrix=fitz.Matrix(scale,scale),annots=True)
                b=wanted.get_pixmap(matrix=fitz.Matrix(scale,scale),annots=True)
                assert (a.width,a.height,a.samples)==(b.width,b.height,b.samples),(label,index,scale,'pixels')
    assert hashes=={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs}
    records.append(dict(label=label,selected=selected,pages=len(order),sha256=hashes))
    if not label.startswith('form-physical-'):
        filled=args.corpus/f'{label}-filled.pdf';pdf=PdfReader(filled)
        hashes[filled.name]=hashlib.sha256(filled.read_bytes()).hexdigest()
        assert pdf.get_fields()['shared']['/V']=='Form linked'
        fields=pdf.trailer['/Root']['/AcroForm']['/Fields'];assert len(fields)==1 and len(fields[0].get_object()['/Kids'])==len(order)
        with fitz.open(filled) as changed,fitz.open(path) as old:
            for a,b in zip(changed,old):
                widgets=list(a.widgets());assert len(widgets)==1 and widgets[0].field_value=='Form linked'
                assert b.get_text().count('Keep field\n')==1
                assert a.get_text()==b.get_text().replace('Keep field\n','Form linked\n')
                filled_pages+=1
assert len(records)==72,len(records)
assert filled_pages==192,filled_pages
report=dict(passed=True,cases=records,filledPages=filled_pages,mupdf=fitz.VersionBind,exactPixelScales=[1,2],
    limitation='Synthetic authored nested-form reference; no broad producer or desktop acceptance implied.')
with args.output.open('x',encoding='utf-8') as out:json.dump(report,out,indent=2);out.write('\n')
print(json.dumps(dict(passed=True,cases=len(records),pages=sum(r['pages'] for r in records))))
