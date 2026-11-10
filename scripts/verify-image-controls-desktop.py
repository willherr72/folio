"""Independent MuPDF/pypdf checks for real desktop image placement outputs."""
from pathlib import Path
import hashlib
import io
import json
import os
import fitz
import numpy as np
import pypdf

root = Path(os.environ.get('FOLIO_IMAGE_ACCEPTANCE_ROOT', 'artifacts/image-controls-desktop')).resolve()
results = json.loads((root / 'results.json').read_text(encoding='utf-8'))
assert results['passed']
source = root / 'unavailable-Quiz.pdf'
original = pypdf.PdfReader(source)
names = ['Editable images.pdf', 'Flattened images.pdf', 'Recovered images.pdf']
readers = {name: pypdf.PdfReader(root / name) for name in names}
stamps = lambda pdf: [ref.get_object() for ref in pdf.pages[0].get('/Annots', [])
                      if ref.get_object().get('/Name') == '/FolioImage']
editable = stamps(readers[names[0]])
recovered = stamps(readers[names[2]])
assert len(editable) == len(recovered) == 3
assert not stamps(readers[names[1]])
records = [json.loads(annot['/Folio'])['overlay'] for annot in editable]
recovered_records = [json.loads(annot['/Folio'])['overlay'] for annot in recovered]
for index, (annot, name) in enumerate(zip(editable, ['Alpha.png', 'Exif.jpg', 'Photo.jpg'])):
    encoded = annot['/FolioImageAsset'].get_object().get_data()
    expected = results['inputHashes'][name]
    assert hashlib.sha256(encoded).hexdigest() == expected
    assert records[index]['assetId'] == expected
assert recovered_records[:2] == records[:2]
assert recovered_records[2]['x'] == 95 and records[2]['x'] == 80
assert {k: v for k, v in recovered_records[2].items() if k != 'x'} == {k: v for k, v in records[2].items() if k != 'x'}
for name, pdf in readers.items():
    assert len(pdf.pages) == len(original.pages)
    for before, after in zip(original.pages, pdf.pages):
        source_text, output_text = before.extract_text(), after.extract_text()
        # pypdf adds one terminal line feed when a flattened image Do follows the
        # final text object. MuPDF text below must remain exactly equal.
        assert output_text in (source_text, source_text+'\n'), (name, 'native text changed')
        assert list(before.mediabox) == list(after.mediabox)
        source_content = before.get_contents().get_data()
        output_content = after.get_contents().get_data()
        if name == 'Flattened images.pdf':
            assert source_content in output_content
        else:
            assert source_content == output_content

def raster(doc, index):
    pix = doc[index].get_pixmap(matrix=fitz.Matrix(2, 2), alpha=False, annots=True)
    return np.frombuffer(pix.samples, np.uint8).reshape(pix.height, pix.width, 3).copy()

pixel_checks = []
# MuPDF chooses a page transparency buffer when an alpha image annotation exists,
# even with annots=False. Remove only our image annotations in an in-memory copy
# to prove that native page pixels remain exactly equal to the original.
without_images = []
for name in ['Editable images.pdf', 'Recovered images.pdf']:
    writer = pypdf.PdfWriter(clone_from=readers[name])
    for page in writer.pages:
        if '/Annots' in page:
            page[pypdf.generic.NameObject('/Annots')] = pypdf.generic.ArrayObject([
                ref for ref in page['/Annots'] if ref.get_object().get('/Name') != '/FolioImage'])
    buffer = io.BytesIO()
    writer.write(buffer)
    without_images.append(fitz.open(stream=buffer.getvalue(), filetype='pdf'))
with fitz.open(source) as source_doc, fitz.open(root / names[0]) as editable_doc, fitz.open(root / names[1]) as flat_doc, fitz.open(root / names[2]) as recovered_doc:
    for index in range(len(source_doc)):
        assert source_doc[index].get_text() == editable_doc[index].get_text() == flat_doc[index].get_text() == recovered_doc[index].get_text()
        before, edit, flat, recovered_pixels = [raster(doc, index) for doc in [source_doc, editable_doc, flat_doc, recovered_doc]]
        assert before.shape == edit.shape == flat.shape == recovered_pixels.shape
        assert np.max(np.abs(edit.astype(int) - flat.astype(int))) <= 1, 'editable/flattened pixels differ'
        outside = np.ones(before.shape[:2], dtype=bool)
        if index == 0:
            for annot in [*editable, *recovered]:
                x0, y0, x1, y1 = map(float, annot['/Rect'])
                height = float(original.pages[0].mediabox.height)
                outside[max(0, int((height-y1)*2)-2):int((height-y0)*2)+3,
                        max(0, int(x0*2)-2):int(x1*2)+3] = False
        for native_only in without_images:
            assert np.array_equal(before, raster(native_only, index)), 'native drawing changed after removing our image annotations'
        outside_difference = max(int(np.max(np.abs(before[outside].astype(int)-pixels[outside].astype(int)))) for pixels in [edit, flat, recovered_pixels])
        assert outside_difference <= 2, 'surrounding pixels changed beyond transparency rounding'
        pixel_checks.append({'page': index+1, 'editableVsFlattenedMaxChannelDifference': int(np.max(np.abs(edit.astype(int)-flat.astype(int)))),
                             'nativePixelsWithImageAnnotationsRemovedEqual': True, 'outsideImageFramesMaxChannelDifference': outside_difference})
for native_only in without_images:
    native_only.close()
report = {'passed': True, 'mupdf': fitz.VersionBind, 'pypdf': pypdf.__version__,
          'originalImageBytesPreserved': True, 'originalDrawingBytesPreserved': True, 'nativeTextUnchanged': True, 'pixelChecks': pixel_checks,
          'rasterNote': 'Alpha image annotations cause MuPDF page-buffer rounding of up to 2/255 outside image frames. Removing only those annotations restores exact native pixels. pypdf may add one terminal line feed after flattened image Do operators; MuPDF text is exactly unchanged.',
          'pdfs': {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in names}}
(root / 'independent-results.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
print(json.dumps(report, indent=2))
