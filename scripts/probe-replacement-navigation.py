"""Create public synthetic cross-page interactions for replacement feasibility."""
import importlib.util
from pathlib import Path
from pypdf import PdfReader, PdfWriter
from pypdf.generic import ArrayObject, DictionaryObject, NameObject, TextStringObject, NumberObject

spec=importlib.util.spec_from_file_location('probe',Path(__file__).with_name('probe-page-replacement.py'))
probe=importlib.util.module_from_spec(spec);spec.loader.exec_module(probe)
root=Path('artifacts/page-replacement-navigation-20261024')
root.mkdir(parents=True,exist_ok=False)
probe.destination(root/'base.pdf',0)
writer=PdfWriter(clone_from=root/'base.pdf')
page0,page1=writer.pages
widget=page0['/Annots'][0].get_object()
field=DictionaryObject({NameObject(key):widget.pop(key) for key in ['/FT','/T','/V','/DA']})
field_ref=writer._add_object(field)
widget[NameObject('/Parent')]=field_ref
other=DictionaryObject(dict(widget));other[NameObject('/P')]=page1.indirect_reference
other_ref=writer._add_object(other)
page1[NameObject('/Annots')]=ArrayObject([other_ref])
field[NameObject('/Kids')]=ArrayObject([page0['/Annots'][0],other_ref])
writer._root_object['/AcroForm'].get_object()[NameObject('/Fields')]=ArrayObject([field_ref])
for index,page in enumerate(writer.pages):
    target=writer.pages[1-index].indirect_reference
    for named in [False,True]:
        destination=ArrayObject([target,NameObject('/Fit')])
        annot=probe.dictionary(Type=NameObject('/Annot'),Subtype=NameObject('/Link'),P=page.indirect_reference,
            Rect=probe.array([30,120 if named else 150,180,140 if named else 170]),Border=probe.array([0,0,0]))
        if named:
            key=f'page{1-index}'
            annot[NameObject('/A')]=probe.dictionary(S=NameObject('/GoTo'),D=TextStringObject(key))
        else:
            annot[NameObject('/Dest')]=destination
        page['/Annots'].append(writer._add_object(annot))
writer._root_object[NameObject('/Names')]=probe.dictionary(Dests=probe.dictionary(Names=ArrayObject([
    TextStringObject('page0'),ArrayObject([page0.indirect_reference,NameObject('/Fit')]),
    TextStringObject('page1'),ArrayObject([page1.indirect_reference,NameObject('/Fit')])
])))
writer.write(root/'target.pdf')
probe.replacement(root/'incoming.pdf','standard',0)
assert len(PdfReader(root/'target.pdf').get_fields())==1
print(root)
