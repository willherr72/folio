"""Feasibility only: fit replacement content while retaining destination interaction graphs.

Produces synthetic inputs, an isolated Form placement and a separately authored
direct-content reference. This is not the product replacement implementation.
"""
import argparse
import hashlib
import json
from pathlib import Path

import fitz
from pypdf import PdfReader, PdfWriter
from pypdf.generic import (ArrayObject, BooleanObject, ContentStream, DecodedStreamObject,
                           DictionaryObject, FloatObject, NameObject, NumberObject,
                           RectangleObject, TextStringObject)


def dictionary(**values):
    return DictionaryObject({NameObject('/' + key): value for key, value in values.items()})


def array(values):
    return ArrayObject([FloatObject(v) for v in values])


def stream(writer, content, **values):
    result = DecodedStreamObject()
    result.update(dictionary(**values))
    result.set_data(content)
    return writer._add_object(result)


def destination(path, rotation):
    writer = PdfWriter()
    page = writer.add_blank_page(300, 400)
    page.cropbox = RectangleObject([10, 20, 290, 380])
    page.rotation = rotation
    font = writer._add_object(dictionary(Type=NameObject('/Font'), Subtype=NameObject('/Type1'), BaseFont=NameObject('/Helvetica')))
    resources = dictionary(Font=dictionary(F1=font))
    page[NameObject('/Resources')] = resources
    page[NameObject('/Contents')] = stream(writer, b'BT /F1 20 Tf 40 220 Td (Old page content) Tj ET')
    appearance = stream(writer, b'0 0 180 26 re S BT /F1 12 Tf 4 8 Td (Keep field) Tj ET',
                        Type=NameObject('/XObject'), Subtype=NameObject('/Form'),
                        BBox=array([0, 0, 180, 26]), Resources=resources)
    widget = writer._add_object(dictionary(Type=NameObject('/Annot'), Subtype=NameObject('/Widget'),
        FT=NameObject('/Tx'), T=TextStringObject('retained'), V=TextStringObject('Keep field'),
        DA=TextStringObject('/F1 12 Tf 0 g'), Rect=array([30, 45, 210, 71]), F=NumberObject(4),
        P=page.indirect_reference, AP=dictionary(N=appearance)))
    note_ap = stream(writer, b'1 0.8 0 rg 0 0 18 18 re f 0 g BT /F1 10 Tf 3 4 Td (N) Tj ET',
                     Type=NameObject('/XObject'), Subtype=NameObject('/Form'),
                     BBox=array([0, 0, 18, 18]), Resources=resources)
    note = writer._add_object(dictionary(Type=NameObject('/Annot'), Subtype=NameObject('/Text'),
        Rect=array([240, 50, 258, 68]), Contents=TextStringObject('Keep review'),
        NM=TextStringObject('retained-note'), T=TextStringObject('Reviewer'),
        F=NumberObject(4), C=array([1, 0.8, 0]), AP=dictionary(N=note_ap), P=page.indirect_reference))
    link = writer._add_object(dictionary(Type=NameObject('/Annot'), Subtype=NameObject('/Link'),
        Rect=array([30, 90, 180, 110]), Border=array([0, 0, 0]), P=page.indirect_reference,
        A=dictionary(S=NameObject('/URI'), URI=TextStringObject('https://example.invalid/retained'))))
    page[NameObject('/Annots')] = ArrayObject([widget, note, link])
    writer._root_object[NameObject('/AcroForm')] = writer._add_object(dictionary(
        Fields=ArrayObject([widget]), DA=TextStringObject('/F1 12 Tf 0 g'), DR=resources,
        NeedAppearances=BooleanObject(False)))
    second = writer.add_blank_page(300, 400)
    second[NameObject('/Resources')] = resources
    second[NameObject('/Contents')] = stream(writer, b'BT /F1 20 Tf 40 220 Td (Untouched second page) Tj ET')
    writer.add_outline_item('Keep target', 0)
    writer.add_outline_item('Keep second', 1)
    writer.write(path)


def replacement(path, kind, rotation):
    if kind == 'embedded':
        writer = PdfWriter(clone_from='tests/fixtures/embedded-text/reportlab-subset.pdf')
        page = writer.pages[0]
        page.cropbox = RectangleObject([10, 20, 290, 380])
    else:
        writer = PdfWriter()
        page = writer.add_blank_page(500, 260)
        page.cropbox = RectangleObject([20, 30, 480, 240])
        font = writer._add_object(dictionary(Type=NameObject('/Font'), Subtype=NameObject('/Type1'), BaseFont=NameObject('/Courier')))
        page[NameObject('/Resources')] = dictionary(Font=dictionary(F1=font))
        page[NameObject('/Contents')] = stream(writer,
            b'0.2 0.5 0.8 RG 2 w 20 30 460 210 re S BT /F1 24 Tf 0.1 0.3 0.7 rg 65 170 Td (Replacement content) Tj ET')
    page.rotation = rotation
    writer.write(path)


def displayed(point, box, rotation):
    x, y = point
    left, bottom, right, top = box
    return {0: (x-left, top-y), 90: (y-bottom, x-left),
            180: (right-x, y-bottom), 270: (top-y, right-x)}[rotation]


def raw(point, box, rotation):
    x, y = point
    left, bottom, right, top = box
    return {0: (left+x, top-y), 90: (left+y, bottom+x),
            180: (right-x, bottom+y), 270: (right-y, top-x)}[rotation]


def placement(source, target):
    a, b = list(map(float, source.cropbox)), list(map(float, target.cropbox))
    size = lambda box, rotation: (box[3]-box[1], box[2]-box[0]) if rotation % 180 else (box[2]-box[0], box[3]-box[1])
    sw, sh = size(a, source.rotation)
    tw, th = size(b, target.rotation)
    scale = min(tw/sw, th/sh)
    def convert(x, y):
        x, y = displayed((x, y), a, source.rotation)
        return raw(((tw-sw*scale)/2+x*scale, (th-sh*scale)/2+y*scale), b, target.rotation)
    origin, x, y = convert(0, 0), convert(1, 0), convert(0, 1)
    return [x[0]-origin[0], x[1]-origin[1], y[0]-origin[0], y[1]-origin[1], *origin]


def compose(destination_path, source_path, output, direct):
    writer = PdfWriter(clone_from=destination_path)
    target = writer.pages[0]
    reader = PdfReader(source_path)
    source = reader.pages[0]
    matrix = placement(source, target)
    prefix = ('q ' + ' '.join(f'{value:.10g}' for value in matrix) + ' cm\n').encode()
    resources = source['/Resources'].clone(writer).get_object()
    if direct:
        content = ContentStream(source.get_contents(), reader)
        font_names = {}
        for i, (name, font) in enumerate(resources['/Font'].items()):
            new_name = NameObject(f'/Incoming{i}')
            font_names[name] = new_name
            target['/Resources']['/Font'][new_name] = font
        for operands, operator in content.operations:
            if operator == b'Tf':
                operands[0] = font_names[operands[0]]
        left, bottom, right, top = map(float, source.cropbox)
        clip = f'{left:g} {bottom:g} {right-left:g} {top-bottom:g} re W n\n'.encode()
        target[NameObject('/Contents')] = stream(writer, prefix + clip + content.get_data() + b'\nQ')
    else:
        form = stream(writer, source.get_contents().get_data(), Type=NameObject('/XObject'),
                      Subtype=NameObject('/Form'), BBox=array(source.cropbox), Resources=resources)
        target['/Resources'][NameObject('/XObject')] = dictionary(FolioReplacement=form)
        target[NameObject('/Contents')] = stream(writer, prefix + b'/FolioReplacement Do\nQ')
    writer.write(output)
    return matrix


def metadata(path):
    reader = PdfReader(path)
    page = reader.pages[0]
    annotations = [a.get_object() for a in page['/Annots']]
    return dict(fields={k: str(v['/V']) for k, v in reader.get_fields().items()},
                comments=[str(a['/Contents']) for a in annotations if a['/Subtype'] == '/Text'],
                links=[str(a['/A']['/URI']) for a in annotations if a['/Subtype'] == '/Link'],
                outlines=[(o.title, reader.get_destination_page_number(o)) for o in reader.outline],
                crop=list(page.cropbox), rotation=page.rotation, pages=len(reader.pages))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    records = []
    for target_rotation in [0, 90, 180, 270]:
        target = args.output / f'destination-{target_rotation}.pdf'
        destination(target, target_rotation)
        for kind in ['standard', 'embedded']:
            for source_rotation in [0, 90, 180, 270]:
                label = f'{kind}-d{target_rotation}-s{source_rotation}'
                source = args.output / f'{label}-source.pdf'
                replacement(source, kind, source_rotation)
                original_hashes = [digest(target), digest(source)]
                actual, reference = [args.output / f'{label}-{suffix}.pdf' for suffix in ['actual', 'reference']]
                matrix = compose(target, source, actual, False)
                compose(target, source, reference, True)
                assert metadata(actual) == metadata(reference) == metadata(target)
                with fitz.open(actual) as a, fitz.open(reference) as b, fitz.open(target) as before:
                    for scale in [2, 1000/a[0].rect.width]:
                        assert a[0].get_pixmap(matrix=fitz.Matrix(scale,scale)).samples == b[0].get_pixmap(matrix=fitz.Matrix(scale,scale)).samples, label
                    assert a[0].get_text() == b[0].get_text(), label
                    assert 'Old page content' not in a[0].get_text()
                    assert a[1].get_pixmap().samples == before[1].get_pixmap().samples
                assert [digest(target), digest(source)] == original_hashes
                records.append(dict(label=label, matrix=matrix, target=target.name, source=source.name,
                                    actual=actual.name, reference=reference.name, passed=True,
                                    sha256={p.name:digest(p) for p in [target,source,actual,reference]}))
    result = dict(passed=True, feasibilityOnly=True, cases=records, mupdf=fitz.VersionBind)
    (args.output/'independent.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(dict(passed=True,feasibilityOnly=True,cases=len(records))))


if __name__ == '__main__':
    main()
