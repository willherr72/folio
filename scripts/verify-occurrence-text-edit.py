"""Independent text, interaction and two-scale raster oracle for native text edits."""
import argparse
import hashlib
import io
import json
from pathlib import Path

import fitz
from pypdf import PdfReader, PdfWriter
from pypdf.generic import DecodedStreamObject, NameObject

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--corpus', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
records = []
for path in sorted(args.corpus.glob('text-*-result.pdf')):
    label = path.name.removesuffix('-result.pdf')
    original = args.corpus / f'{label}-source.pdf'
    hashes = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in [path, original]}
    if label == 'text-unique':
        order, selected, replacement = [0, 1], 0, b'Next content'
    elif label in ['text-first', 'text-later']:
        order, selected, replacement = [0, 1, 0], (0 if label == 'text-first' else 2), b'Next content'
    else:
        order, selected, replacement = [0, 1, 0], 2, b'New content'
    before, actual = PdfReader(original), PdfReader(path)
    assert len(actual.pages) == len(order), label
    fields = actual.trailer['/Root']['/AcroForm']['/Fields']
    assert len(fields) == 1 and len(fields[0].get_object()['/Kids']) == len(order), label
    assert list(actual.get_fields()) == ['shared']
    assert actual.get_fields()['shared']['/V'] == 'Keep field'
    ids = [page.indirect_reference for page in actual.pages]
    targets = []
    for index, old in enumerate(order):
        page, source_page = actual.pages[index], before.pages[old]
        assert page.mediabox == source_page.mediabox and page.cropbox == source_page.cropbox
        assert page.rotation == source_page.rotation
        assert float(page.get('/UserUnit', 1)) == float(source_page.get('/UserUnit', 1))
        links = {}
        for ref in page['/Annots']:
            annotation = ref.get_object()
            if annotation['/Subtype'] == '/Widget':
                assert annotation.raw_get('/Parent') == fields[0]
            elif annotation['/Subtype'] == '/Text':
                assert annotation['/Contents'] == 'Keep review'
            elif annotation['/Subtype'] == '/Link':
                name = str(annotation['/NM'])
                dest = annotation['/A']['/D'] if name == 'named' else annotation['/Dest']
                if isinstance(dest, str):
                    target = actual.get_destination_page_number(actual.named_destinations[dest])
                else:
                    target = ids.index(dest[0])
                assert target == (index if name == 'self' else order.index(1 - old)), (label, index, name, target)
                links[name] = target
        assert set(links) == {'direct', 'named', 'self'}
        targets.append(links)
    # Independently replace the fixture's literal text without using Folio output.
    writer = PdfWriter(clone_from=before)
    page = writer.pages[0]
    content = page.get_contents().get_data()
    assert content.count(b'(Old content)') == 1
    stream = DecodedStreamObject()
    stream.set_data(content.replace(b'(Old content)', b'(' + replacement + b')'))
    page[NameObject('/Contents')] = writer._add_object(stream)
    reference = io.BytesIO()
    writer.write(reference)
    with fitz.open(original) as old, fitz.open(path) as changed, fitz.open(stream=reference.getvalue(), filetype='pdf') as expected:
        for index, old_index in enumerate(order):
            wanted = expected[0] if index == selected else old[old_index]
            assert changed[index].get_text() == wanted.get_text(), (label, index, 'text')
            for scale in [1, 2]:
                a = changed[index].get_pixmap(matrix=fitz.Matrix(scale, scale))
                b = wanted.get_pixmap(matrix=fitz.Matrix(scale, scale))
                assert (a.width, a.height, a.samples) == (b.width, b.height, b.samples), (label, index, scale, 'pixels')
    assert hashes == {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in [path, original]}
    records.append(dict(label=label, selected=selected, pages=len(order), targets=targets, sha256=hashes))
assert len(records) == 15, len(records)
report = dict(passed=True, cases=records, mupdf=fitz.VersionBind,
              checks='Shared field identity and value, notes, direct/named/self links, raw frames and UserUnit, independent exact text/pixels at 1x/2x, source integrity')
with args.output.open('x', encoding='utf-8') as output:
    json.dump(report, output, indent=2)
    output.write('\n')
print(json.dumps(dict(passed=True, cases=len(records), pages=sum(r['pages'] for r in records))))
