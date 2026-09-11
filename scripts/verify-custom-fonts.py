"""Independent inspection of owned packaged custom-font smoke PDFs."""
from pathlib import Path
import hashlib,json
import fitz
from pypdf import PdfReader
root=Path("artifacts/custom-font-desktop")
results=json.loads((root/"results.json").read_text())
sha=lambda data:hashlib.sha256(data).hexdigest()
assert sha((root/"Moved original.pdf").read_bytes())==results["originalSha256"]
expected="Custom café Ω Ж"
a=PdfReader(root/"Editable.pdf");b=PdfReader(root/"Flattened.pdf")
annotations=a.pages[0]["/Annots"]
assert len(annotations)==1
note=annotations[0].get_object();assert note["/Contents"]==expected
font=note["/AP"]["/N"]["/Resources"]["/Font"]["/FolioFont"]
assert font["/Subtype"]=="/Type0"
child=font["/DescendantFonts"][0].get_object();assert child["/Subtype"]=="/CIDFontType2"
program=child["/FontDescriptor"]["/FontFile2"].get_data()
assert sha(program)==results["fontSha256"]
assert program==(root/"Moved font.ttf").read_bytes()
assert expected in b.pages[0].extract_text()
assert "BASE CONTENT" in b.pages[0].extract_text()
assert not b.pages[0].get("/Annots")
editable=fitz.open(root/"Editable.pdf");flat=fitz.open(root/"Flattened.pdf")
assert expected in flat[0].get_text()
pa=editable[0].get_pixmap(matrix=fitz.Matrix(1.5,1.5),alpha=False)
pb=flat[0].get_pixmap(matrix=fitz.Matrix(1.5,1.5),alpha=False)
assert (pa.width,pa.height)==(pb.width,pb.height)
pixels_a,pixels_b=pa.samples,pb.samples
changed=sum(any(abs(x-y)>8 for x,y in zip(pixels_a[i:i+3],pixels_b[i:i+3])) for i in range(0,len(pixels_a),3))
assert changed<100,changed
pa.save(root/"independent-render.png")
report={"passed":True,"readers":["pypdf","PyMuPDF"],"embeddedFontSha256":sha(program),"embeddedBytes":len(program),"pypdfText":b.pages[0].extract_text(),"mupdfText":flat[0].get_text(),"differentPixelsEditableVsFlattened":changed,"originalUnchanged":True}
(root/"independent-verification.json").write_text(json.dumps(report,indent=2,ensure_ascii=False)+"\n",encoding="utf-8")
print(json.dumps(report,indent=2,ensure_ascii=False))
