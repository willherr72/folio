# Semantic Type3 prototype: independent geometry review

The prototype improves exact Unicode and PDFium cluster selection for the
bounded unmixed samples. It does **not** establish portable cluster selection
geometry across readers. Mixed bidi remains a four-rotation negative control.
These conclusions concern the Python prototype artifacts, not a reviewed native
serializer or a production UI feature.

Reproduce with `python scripts/inspect-semantic-geometry.py`. Inputs are
`artifacts/shaped-text/semantic-rotations/{semantic.pdf,inputs.json,inspection}`
and the original native layout JSON. The resulting `geometry-review.json`
retains every compared PDFium box, native cluster ink comparison, MuPDF default
and alternate-option boxes, structure checks, and preservation results.

## What the positive cases establish

The visual-TJ variant has 24 exact-Unicode positive pages: six samples at all
four quarter turns. The four mixed-bidi pages fail exact logical extraction.
The positive PDFium character boxes differ from the intended native cluster
advance-plus-ink union by at most **0.048001 points** at the tested 24-point font
size. Maximum clipping into the independently reconstructed tight native cluster
ink is also **0.048001 points**. This is small metric quantization, not exact
bounding-box equality. Scale-dependent tolerances and long-run accumulation
still need native tests; do not generalize the numeric bound to arbitrary sizes.

The logical source and Unicode units are checked before comparing boxes.
Supplementary U+1D434 yields two PDFium UTF-16 entries sharing the intended
cluster box. Combining characters and ligature interiors similarly receive the
whole cluster extent. These are coarse cluster-selection boxes, not independent
caret positions or independent ink for each Unicode scalar.

All 28 visual-TJ pages contain exactly one Type3 font and one text object.
Every Type3 CharProc consists solely of a six-operand `d1`; none paints paths,
images, or text. Visible glyphs are separate filled vector paths. No ActualText
spans are present. The structure therefore contains one semantic text layer and
does not also emit the visible glyphs as extractable font text.

Before/after PDFium save, the inspected Type3 program structure, PDFium boxes,
MuPDF default character boxes, and MuPDF raster hashes are preserved for all
28 pages. Preservation is distinct from pixel identity to the original hinted
TrueType rendering: converting glyphs to vector paths changes that rendering
mechanism, and this review does not claim identical rasterization against it.

## Portable geometry remains unresolved

MuPDF's default character boxes use advances and the Type3 font-wide vertical
metrics. With this prototype's FontBBox, a 24-point semantic font produces
**72-point-tall** default boxes. They divide a combining cluster into separate
scalar-width cells instead of repeating its full union. Maximum edge difference
from the expected cluster box across the positive cases is **38.832 points**.

For `q\u0307\u0323`, MuPDF reports three separate approximately 5.064-point-wide
cells, each 72 points high. The native cluster ink is only 27.656 points high.
Default selection of one combining scalar therefore does not cover the same
cluster as PDFium selection. Indic and ligature interiors show the same
structural difference.

The [MuPDF structured-text options](https://mupdf.readthedocs.io/en/latest/reference/common/stext-options.html)
offer accurate bounding boxes and side bearings. In the measured PyMuPDF 1.27.1,
`TEXT_ACCURATE_BBOXES` restores the mark cluster's vertical ink range, but still
partitions its horizontal extent. Adding `TEXT_ACCURATE_SIDE_BEARINGS` produces
asymmetric boxes rather than the required repeated full cluster union. Raw
option-specific results are retained in the report; changing an extraction
option is not evidence that ordinary external viewers use that option.

## Architectural limits to preserve in the native gate

- Blank Type3 glyphs advertise semantic rectangles through `d1` while painting
  no ink. PDFium's use of these rectangles is measured behavior, not proof of
  consistent selection or accessibility in every PDF reader.
- Pure and mixed bidi must remain separate tests. Exact Arabic extraction does
  not prove that a mixed Latin/Arabic/digit line preserves logical order.
- A single-byte Type3 encoding needs a strict code-allocation bound. The proposed
  255-scalar limit is a bounded experimental policy, not wrapping or long-text
  support; supplementary scalars and UTF-16 units must not be conflated.
- Source font identity must remain attached to native layout/outline production.
  The exported semantic Type3 font is not the original editable font program and
  must not be treated as proof that existing-text replacement is safe.
- Changing global FontBBox can reduce a coarse line height but cannot alone
  make MuPDF select all glyph ink for each scalar inside a cluster.

The useful claim at this stage is exact Unicode in three measured readers for
the bounded unmixed fixtures, plus approximate native cluster boxes in PDFium.
Cross-reader cluster geometry, mixed bidi, ordinary viewer selection behavior,
accessibility, and production integration remain separate unresolved gates.
