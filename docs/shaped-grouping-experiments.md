# Grouped TJ experiment, 2026-09-11

This experiment modifies only native fixture PDF operator grouping and Unicode
mapping. It uses all 36 native cases (nine samples × four rotations), seven
variants = 252 pages per run. Native HarfRust layout and glyph positions remain
the input. No reader output or source-semantic string is reversed/normalized;
exactness ignores only final CR/LF reader terminators. This is not app support.

Reproduce from the worktree root:

```powershell
python scripts/probe-grouped-shaping.py
python scripts/probe-grouped-shaping.py --integer-widths --output artifacts/shaped-text/grouped/integer-widths
```

Artifacts are under `artifacts/shaped-text/grouped` and its `integer-widths` subdirectory. Each directory contains `grouped.pdf`, a real `FPDF_SaveAsCopy` output
`grouped.resaved.pdf`, and `results.json` with all raw reader strings, PDFium
per-character Unicode/boxes, page rotation, MuPDF2x RGB raster hashes/deltas
against the native page, and preservation results. Readers: bundled PDFium SHA
`04100c03e41cac1f979e36e5e26fb860bcb5a7461f53830d3c098716624a27a9`,
PyMuPDF1.27.1, pypdf6.14.2. Raster measurements here are MuPDF, not PDFium.

## Result

No variant meets exact Unicode plus reliable cluster bounds in both PDFium and
MuPDF. These extraction counts are the same with fractional or integer CID widths:

| Variant | PDFium exact /36 | MuPDF exact /36 | pypdf exact /36 |
| --- | ---: | ---: | ---: |
| Whole logical ActualText, grouped TJ |36|24|24|
| Per-run ActualText, visual run order |32|24|24|
| Per-run ActualText, logical run order |34|24|24|
| Per-cluster ActualText + once-per-cluster ToUnicode, logical cluster order |27|32|32|
| Once-per-cluster ToUnicode, logical cluster order |18|24|32|
| Once-per-cluster ToUnicode, visual cluster order |18|20|28|
| Once-per-cluster ToUnicode, grouped visual runs |23|20|32|

Every mode preserves raw reader strings, PDFium boxes, and MuPDF rasters through
save/reopen (252/252 pages in each run). This preserves failures, not success.

### Geometry

Whole-line TJ now gives the mixed line a full PDFium horizontal extent
(approximately x48.19..261.11 rather than the first painted glyph only). But it
subdivides that extent into16 equal 13.307-point boxes in logical sequence.
The logical Arabic text therefore starts around x101.42, where the visible
numeric run occurs. A correct outer rectangle is not correct selection mapping.

Marks still require multiple text objects because TJ adjustments are horizontal.
The probe splits when native y changes rather than flattening marks. Whole-span
PDFium mark boxes have y 52.454..71.054, while native mark ink extends 48..75.656.
The ActualText geometry therefore continues to miss the independently offset ink.

### Raw extraction examples

With whole TJ and one logical ActualText span, source `سلام` is exact in PDFium
but MuPDF returns `مالس\n`. Source `ABC سلام 123 DEF` is exact in PDFium, but
MuPDF returns `ABC سلام 321 DEF\n` and pypdf returns only `DEF`.

The strongest local improvement is cluster TJ + logical ActualText with each
cluster's Unicode assigned once: `किताब` extracts exactly in all three readers
at 0°,90°,270°. At 180°, PDFium returns `ब ता कि`, while MuPDF/pypdf stay exact.
Arabic under this same strategy becomes `ملاس` in PDFium while MuPDF/pypdf
are exact. Mixed PDFium output becomes `ABC 123 ملاس DEF`; MuPDF contains
interior newlines. It still fails the gate.

Assigning an empty ToUnicode destination to additional glyphs in a cluster does
prevent pypdf duplication. Without ActualText, PDFium/MuPDF can expose raw CID
control scalars instead of suppressing them: Indic includes U+0004 in PDFium,
and U+0002/U+0004 in MuPDF. Empty destinations are a measured compatibility
experiment, not a recommended production encoding.

### TJ metric precision and rendering

The original CID/W widths are fractional. MuPDF quantizes these for implicit TJ
advances: `café`'s final glyph moves from x84.6328125 to 84.624, a 0.0088125-point
error. At 2x rasterization this can change 582 RGB channels with maximum delta 221.
The original per-glyph Tm representation did not rely on implicit advances.

The `--integer-widths` variant emits integral W entries and compensates exactly
with TJ adjustments. This eliminates the grouping drift: whole/run-grouped
variants become 36/36 byte-identical to native MuPDF rasters. Logical cluster
paint reordering still changes up to a few RGB channels by1 because overlapping
antialiased glyphs are composited in a different order; raw detail is in JSON.
No extraction counts improve merely by correcting the widths.

## Useful next criteria

A successful representation must handle logical source order independently of
paint order without relying on reader-specific reordering. Test Arabic lam-alef,
mixed digits, Indic at180°, and two-axis marks first; compare exact source Unicode
and cluster selection bounds, not just full-line bounds or visual appearance.
If TJ is retained for horizontal groups, use explicit integral CID widths with
matching adjustments and preserve vertical mark offsets. Any independent
semantic layer must also demonstrate no duplicated/control text, truthful
cluster geometry, all rotations, unchanged visible pixels, and save/reopen
preservation in PDFium and MuPDF.
