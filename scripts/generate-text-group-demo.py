"""Generate Folio's original adjacent-text practice PDF."""
from pathlib import Path
from pypdf import PdfWriter
from pypdf.generic import DictionaryObject,NameObject,DecodedStreamObject
writer=PdfWriter();page=writer.add_blank_page(width=612,height=792)
font=writer._add_object(DictionaryObject({NameObject('/Type'):NameObject('/Font'),NameObject('/Subtype'):NameObject('/Type1'),NameObject('/BaseFont'):NameObject('/Helvetica')}))
page[NameObject('/Resources')]=DictionaryObject({NameObject('/Font'):DictionaryObject({NameObject('/F1'):font})})
content=b"""0.12 0.16 0.20 rg
BT /F1 25 Tf 54 730 Td (Edit words split into pieces) Tj ET
BT /F1 12 Tf 54 702 Td (Some PDFs store one phrase as several separate pieces.) Tj ET
BT /F1 12 Tf 54 680 Td (Choose Edit text, click a piece, then choose Edit together...) Tj ET
BT /F1 12 Tf 54 660 Td (Select the matching pieces, check the selection, and apply your edit.) Tj ET
BT /F1 10 Tf 54 584 Td (A WORD SPLIT ACROSS PIECES) Tj ET
BT /F1 22 Tf 80 540 Td (In) Tj (voice) Tj ( total) Tj ET
BT /F1 10 Tf 54 434 Td (A PHRASE SPLIT AT A SPACE) Tj ET
BT /F1 22 Tf 80 390 Td (Annual ) Tj (report) Tj ET
BT /F1 10 Tf 54 284 Td (THREE PIECES, ONE REPLACEMENT) Tj ET
BT /F1 22 Tf 80 240 Td (Quarterly) Tj ( sales) Tj ( report) Tj ET
BT /F1 11 Tf 54 124 Td (Try shorter and longer replacements, then Undo and Redo.) Tj ET
BT /F1 11 Tf 54 104 Td (Save a copy and reopen it to search or copy the changed words.) Tj ET
BT /F1 10 Tf 54 76 Td (Folio keeps the original file unchanged.) Tj ET
"""
stream=DecodedStreamObject();stream.set_data(content);page[NameObject('/Contents')]=writer._add_object(stream)
writer.add_metadata({'/Title':'Folio adjacent text practice','/Author':'Folio contributors'})
output=Path(__file__).resolve().parents[1]/'examples/Edit text together.pdf';writer.write(output);print(output)
