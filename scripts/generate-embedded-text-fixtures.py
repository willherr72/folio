"""Generate synthetic owned desktop fixtures using the licensed DejaVu test font."""
from pathlib import Path
import hashlib,json
from reportlab.pdfgen import canvas
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from pypdf import PdfReader,PdfWriter
root=Path("artifacts/embedded-text-desktop");root.mkdir(parents=True,exist_ok=True)
font=Path("tests/fixtures/corpus/fonts/DejaVuSerif.ttf")
pdfmetrics.registerFont(TTFont("FolioEmbeddedProbe",str(font)))
source=root/"Embedded.pdf"
if source.exists(): raise SystemExit("Refusing to overwrite existing desktop fixture")
c=canvas.Canvas(str(source),pagesize=(480,640),pageCompression=0,invariant=1)
c.setFont("FolioEmbeddedProbe",14);c.setFillColorRGB(.1,.2,.3)
c.drawString(40,540,"Original sentence")
c.drawString(40,490,"Résumé café")
c.setFont("Helvetica",12);c.setFillColorRGB(0,0,0);c.drawString(40,440,"Keep this neighbor")
c.showPage();c.save()
reader=PdfReader(source);writer=PdfWriter();writer.add_page(reader.pages[0])
for item in writer.pages[0]["/Resources"]["/Font"].values():
 resource=item.get_object()
 if "/FontDescriptor" in resource: resource.pop("/ToUnicode",None)
with (root/"Missing Unicode.pdf").open("wb") as stream: writer.write(stream)
(root/"fixtures.json").write_text(json.dumps({"sourceSha256":hashlib.sha256(source.read_bytes()).hexdigest(),"fontSha256":hashlib.sha256(font.read_bytes()).hexdigest()},indent=2)+"\n")
print(source)
