"""Generate an original practice PDF with three shared nested form occurrences."""
from pathlib import Path
from pypdf import PdfWriter
from pypdf.generic import DictionaryObject,NameObject,ArrayObject,NumberObject,DecodedStreamObject
w=PdfWriter()
def dictionary(**values): return DictionaryObject({NameObject('/'+k):v for k,v in values.items()})
def name(value): return NameObject('/'+value)
def array(*values): return ArrayObject([NumberObject(value) for value in values])
def stream(content,**values):
 result=DecodedStreamObject();result.set_data(content);result.update(dictionary(**values));return w._add_object(result)
heading=w._add_object(dictionary(Type=name('Font'),Subtype=name('Type1'),BaseFont=name('Helvetica')))
font=w._add_object(dictionary(Type=name('Font'),Subtype=name('Type1'),BaseFont=name('Courier')))
leaf=stream(b'BT /F2 20 Tf 1 0 0 1 12 18 Tm (Shared words) Tj ET',Type=name('XObject'),Subtype=name('Form'),BBox=array(0,0,260,50),Resources=dictionary(Font=dictionary(F2=font)))
panel=stream(b'q 1 0 0 1 12 12 cm /Leaf Do Q',Type=name('XObject'),Subtype=name('Form'),BBox=array(0,0,284,74),Resources=dictionary(XObject=dictionary(Leaf=leaf)))
resources=dictionary(Font=dictionary(F1=heading),XObject=dictionary(Panel=panel))
contents=[b"""BT /F1 25 Tf 54 730 Td (Edit one shared occurrence) Tj ET
BT /F1 12 Tf 54 700 Td (These three placements share the same nested PDF form.) Tj ET
BT /F1 12 Tf 54 680 Td (Choose Edit text and click the first Shared words below.) Tj ET
BT /F1 12 Tf 54 660 Td (Change it to New words. The other copies should stay unchanged.) Tj ET
BT /F1 10 Tf 54 600 Td (FIRST OCCURRENCE - TRY EDITING THIS ONE) Tj ET
q 1 0 0 1 68 500 cm /Panel Do Q
BT /F1 10 Tf 54 410 Td (SECOND OCCURRENCE - SAME SHARED RESOURCE) Tj ET
q 1 0 0 1 68 310 cm /Panel Do Q
BT /F1 12 Tf 54 220 Td (A third placement is on page 2.) Tj ET
BT /F1 12 Tf 54 198 Td (Try Undo and Redo, then save a copy and reopen it.) Tj ET
BT /F1 10 Tf 54 76 Td (Folio keeps the original file unchanged.) Tj ET""",b"""BT /F1 25 Tf 54 730 Td (The third shared occurrence) Tj ET
BT /F1 12 Tf 54 700 Td (Editing page 1 must leave this placement unchanged.) Tj ET
q 1 0 0 1 68 500 cm /Panel Do Q
BT /F1 12 Tf 54 380 Td (The visible words live two form levels below each page.) Tj ET
BT /F1 12 Tf 54 358 Td (Folio isolates the selected placement before changing its text.) Tj ET"""]
for content in contents:
 page=w.add_blank_page(width=612,height=792);page[name('Resources')]=resources;page[name('Contents')]=stream(content)
w.add_metadata({'/Title':'Folio isolated form text practice','/Author':'Folio contributors'})
output=Path(__file__).resolve().parents[1]/'examples/Edit one shared occurrence.pdf'
w.write(output);print(output)
