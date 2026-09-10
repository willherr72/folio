"""Independent-reader verification of artifacts produced by native persistence tests.
Run: python -X utf8 scripts/verify-persistence-pdf.py artifacts/annotation-tests
Requires PyMuPDF, pypdf and Pillow (development only). No PDFs are modified.
"""
from pathlib import Path
import argparse,json,hashlib
import fitz
from PIL import Image,ImageChops
from pypdf import PdfReader

def mask(image,channel):
    channels=image.split()[:3]
    other=[value for index,value in enumerate(channels) if index!=channel]
    return ImageChops.subtract(channels[channel],ImageChops.lighter(*other)).point(lambda value:255 if value>40 else 0)

def compare(a,b):
    union=ImageChops.lighter(a,b).histogram()[255]
    intersection=ImageChops.darker(a,b).histogram()[255]
    assert union>0,'Expected visible colored addition'
    abox,bbox=a.getbbox(),b.getbbox()
    assert abox and bbox,'An addition disappeared in one representation'
    return {'editablePixels':a.histogram()[255],'flatPixels':b.histogram()[255],
            'iou':intersection/union,'editableBounds':abox,'flatBounds':bbox,
            'maxBoundDelta':max(abs(x-y) for x,y in zip(abox,bbox))}

def render(path):
    with fitz.open(path) as pdf:
        page=pdf[0];pix=page.get_pixmap(matrix=fitz.Matrix(2,2),alpha=False,annots=True)
        return Image.frombytes('RGB',(pix.width,pix.height),pix.samples)

def main():
    parser=argparse.ArgumentParser();parser.add_argument('directory',type=Path);args=parser.parse_args()
    rows=[]
    for saved in sorted(args.directory.glob('saved-*-*-*.pdf')):
        flat=saved.with_name(saved.name.replace('saved-','flat-',1))
        assert flat.exists(),f'Missing paired flattened PDF: {flat}'
        pdf=PdfReader(saved);annotations=[ref.get_object() for ref in pdf.pages[0]['/Annots']]
        assert sorted(str(a['/Subtype']) for a in annotations)==['/FreeText','/Ink']
        for annotation in annotations:
            assert '/Folio' in annotation and '/AP' in annotation
            ap=annotation['/AP']['/N'].get_object()
            assert len(ap.get_data())>0 and '/BBox' in ap and '/Resources' in ap
        assert '/EmbeddedFiles' not in pdf.trailer['/Root'].get('/Names',{})
        flat_pdf=PdfReader(flat)
        assert not flat_pdf.pages[0].get('/Annots'),'Flattened fixture should have no text/ink annotations'
        a,b=render(saved),render(flat);assert a.size==b.size
        stats={name:compare(mask(a,channel),mask(b,channel)) for name,channel in [('text',0),('ink',2)]}
        # PDFium and a standard Helvetica AP differ slightly at glyph/stroke edges.
        # Compare each color separately so intact ink cannot hide missing text.
        for name,stat in stats.items():
            assert stat['iou']>=0.90,(saved.name,name,stat)
            assert stat['maxBoundDelta']<=2,(saved.name,name,stat)
        rows.append({'file':saved.name,'sha256':hashlib.sha256(saved.read_bytes()).hexdigest(),**stats})
    assert len(rows)==64,f'Expected 64 source/page/text rotation cases, got {len(rows)}'
    result={'passed':True,'pairs':len(rows),'renderer':fitz.VersionBind,'scale':2,
            'method':'Each text and ink color mask must overlap at least90% and its visible bounds agree within2 raster pixels. Standard indirect annotations/AP resources inspected by pypdf.',
            'minimumTextIoU':min(row['text']['iou'] for row in rows),'minimumInkIoU':min(row['ink']['iou'] for row in rows),'cases':rows}
    (args.directory/'independent-persistence.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(json.dumps({key:value for key,value in result.items() if key!='cases'},indent=2))
if __name__=='__main__':main()