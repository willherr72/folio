"""Original, redistributable ReportLab/Platypus and MuPDF form paragraph inputs."""
from pathlib import Path
import hashlib,json,sys
import fitz,reportlab
from reportlab.pdfgen import canvas
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import Paragraph
from reportlab.lib.styles import ParagraphStyle

root=Path(sys.argv[1] if len(sys.argv)>1 else 'tests/fixtures/form-paragraph');root.mkdir(parents=True,exist_ok=True)
if list(root.iterdir()):raise RuntimeError('Preserving existing fixture directory; choose a new output folder')
font=Path('tests/fixtures/corpus/fonts/DejaVuSerif.ttf');pdfmetrics.registerFont(TTFont('FixtureSerif',str(font)))
for mode in ['standard','embedded','mixed']:
 c=canvas.Canvas(str(root/f'{mode}.pdf'),pagesize=(800,500),invariant=1,pageCompression=1)
 family='FixtureSerif' if mode=='embedded' else 'Helvetica'
 middle='<font name="FixtureSerif" color="#225588">Café colored</font>' if mode=='mixed' else '<font color="#225588">colored</font>'
 paragraph=Paragraph(f'First paragraph line<br/>Second {middle} paragraph line',ParagraphStyle('fixture',fontName=family,fontSize=12,leading=16))
 width,height=paragraph.wrap(240,100)
 c.beginForm('Leaf',0,0,260,120);paragraph.drawOn(c,5,100-height);c.endForm()
 c.beginForm('Wrapper',0,0,260,120);c.doForm('Leaf');c.endForm()
 for x in [40,400]:c.saveState();c.translate(x,220);c.doForm('Wrapper');c.restoreState()
 c.showPage();c.setFont('Helvetica',12);c.drawString(30,100,'Other page unchanged');c.save()
source=fitz.open(root/'mixed.pdf');imported=fitz.open()
for page in source:
 dest=imported.new_page(width=800,height=500);dest.show_pdf_page(dest.rect,source,page.number)
imported.set_metadata({'producer':'Folio synthetic corpus / PyMuPDF show_pdf_page'})
imported.save(root/'imported.pdf',deflate=True,no_new_id=True);imported.close();source.close()
manifest={'generator':'scripts/generate-form-paragraph-fixtures.py','reportlab':reportlab.Version,'pymupdf':fitz.VersionBind,'source':'Original locally authored text. ReportLab Paragraph layout in shared nested forms; MuPDF imported-page wrapper.','font':str(font),'fontSha256':hashlib.sha256(font.read_bytes()).hexdigest(),'license':'tests/fixtures/corpus/fonts/LICENSE_DEJAVU.txt','files':{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.glob('*.pdf'))}}
(root/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print('Generated four producer fixtures')
