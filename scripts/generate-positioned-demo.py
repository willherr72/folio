"""Generate Folio's original, redistributable positioned-text practice PDF."""
from pathlib import Path
from pypdf import PdfWriter
from pypdf.generic import DictionaryObject,NameObject,DecodedStreamObject
writer=PdfWriter(); page=writer.add_blank_page(width=612,height=792)
font=writer._add_object(DictionaryObject({NameObject('/Type'):NameObject('/Font'),NameObject('/Subtype'):NameObject('/Type1'),NameObject('/BaseFont'):NameObject('/Helvetica')}))
page[NameObject('/Resources')]=DictionaryObject({NameObject('/Font'):DictionaryObject({NameObject('/F1'):font})})
content=b"""0.12 0.16 0.20 rg
BT /F1 26 Tf 54 730 Td (Try editing positioned text) Tj ET
BT /F1 12 Tf 54 701 Td (Choose Edit text, click a sample below, and replace its words.) Tj ET
BT /F1 12 Tf 54 682 Td (Try a shorter phrase, then a longer one. Undo and redo each edit.) Tj ET
BT /F1 10 Tf 54 631 Td (ROTATED BASELINE) Tj ET
q BT /F1 18 Tf 0.9659258 0.258819 -0.258819 0.9659258 80 568 Tm (Rotated words) Tj ET Q
BT /F1 10 Tf 54 499 Td (SKEW AND HORIZONTAL SCALE) Tj ET
q BT /F1 18 Tf 1.1 0 0.25 1 80 458 Tm (Skewed words) Tj ET Q
BT /F1 10 Tf 54 389 Td (CHARACTER AND WORD SPACING) Tj ET
q BT /F1 18 Tf 0.7 Tc 2 Tw 80 348 Td (Spaced words) Tj ET Q
BT /F1 10 Tf 54 279 Td (EXPLICIT CHARACTER POSITIONING) Tj ET
q BT /F1 18 Tf 80 238 Td [(Positioned) 40 ( words)] TJ ET Q
BT /F1 11 Tf 54 120 Td (Save a copy, reopen it, and try searching or copying your new text.) Tj ET
BT /F1 10 Tf 54 100 Td (Folio preserves the original file. Text will not automatically wrap.) Tj ET
"""
stream=DecodedStreamObject();stream.set_data(content);page[NameObject('/Contents')]=writer._add_object(stream)
writer.add_metadata({'/Title':'Folio positioned text practice','/Author':'Folio contributors'})
output=Path(__file__).resolve().parents[1]/'examples/Positioned text demo.pdf'
writer.write(output); print(output)
