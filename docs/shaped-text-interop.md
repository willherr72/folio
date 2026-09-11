# Issue #14: font fixtures and reader interoperability gate

This is development evidence for a proposed representation, not implemented
application script support. The first direct-PDF experiment fails the required
independent-reader gate. Preserve the current production character policy until
a native-produced representation passes extraction and geometry checks.

## Reproduce

Verify the licensed static fonts with
`python scripts/fetch-shaped-text-fixtures.py`. Their full licenses, immutable
source URLs, sizes, hashes, and tested coverage are in
[`tests/fixtures/shaped-text`](../tests/fixtures/shaped-text/README.md).

Install the isolated Python dependencies as documented in
`scripts/probe-complex-text.py`, then run:

```powershell
python scripts/probe-shaped-text-interop.py --python-deps artifacts/complex-text/python
```

For the September 11, 2026 measurement, the isolated dependency directory from
the sibling `font-editing` worktree was passed explicitly. Versions were
uharfbuzz 0.55.0 (HarfBuzz 14.2.1), python-bidi 0.6.11, fontTools 4.62.1,
PyMuPDF 1.27.1, and pypdf 6.14.2. The actual bundled Windows PDFium DLL SHA256
was `04100c03e41cac1f979e36e5e26fb860bcb5a7461f53830d3c098716624a27a9`.
Results are under `artifacts/shaped-text/interop/`: original PDF, PDFium-resaved
PDF, and `results.json` containing raw strings, character boxes, glyph/cluster
inputs, font identities, reader versions, and raster hashes.

The research shaper uses a fixed-sample legacy Python bidi implementation.
It is not Folio's HarfRust layout and is not a Unicode conformance runner.
The PDF writer embeds exact fixture bytes, allocates one CID per glyph
occurrence, maps each CID to the complete HarfBuzz cluster through ToUnicode,
and emits each positioned glyph through an absolute text matrix and `Tj`.
Four variants compare no ActualText, one logical ActualText span, logical text
per visual run, and logical text per run emitted in logical run order while
retaining visual positions. No mapping or extracted string is reversed,
normalized, trimmed internally, or corrected to make a comparison pass.

## Measured result: whole logical ActualText span

MuPDF appends one final line feed in these pages; the table omits only that
reader terminator. JSON preserves the complete raw output.

| Source | PDFium | MuPDF | pypdf |
| --- | --- | --- | --- |
| `office` | Exact | Exact | Exact |
| `q\u0307\u0323` | Exact | Exact | Duplicates both combining marks |
| `A\U0001D434B` | Exact | Exact | Exact |
| `سلام` | Exact | `مالس` (reversed scalar order) | Exact |
| `किताब` | Exact | `किताबताब` (duplicated suffix) | `किकिताब` (duplicated first cluster) |
| `ABC سلام 123 DEF` | Exact | `ABC سلام321  DEF` | `ABC 123سلامDEF` |

Per-run ActualText did not repair mixed text: PDFium yielded visual run order
`ABC 123 سلام DEF`; MuPDF omitted Arabic and inserted line breaks. Merely
emitting those runs in logical content order did not restore logical extraction.
The no-ActualText negative control duplicates multi-glyph cluster text for marks
and Indic. It also exposes Arabic lam-alef ordering disagreement: PDFium and
MuPDF produce `سالم`, while pypdf produces the intended `سلام`.

All 24 research pages preserved identical raw text in each of the three readers
after actual `FPDF_SaveAsCopy` and reopen. All 24 also preserved byte-identical
MuPDF RGB raster hashes at 2x scale. This verifies preservation of the candidate,
including its failures. It does not prove visual correctness against independent
shaping, PDFium raster equivalence, page-content regeneration, or rotations.

### Text geometry is a separate blocker

For the mixed whole-span page, PDFium returns all 16 logical character boxes
inside the first `A` glyph's x range **32.288–56.336**, although visible glyphs
extend to **351.883**. For Arabic, all four boxes occupy only the first emitted
glyph's **32.720–47.984** range; the actual run reaches **97.412**. The marks
variant similarly loses the independently offset mark ink from its text boxes.

This agrees with PDFium's implementation: its ActualText handling synthesizes
character boxes from the current text object's rectangle and skips subsequent
objects sharing the same marked-content parameters. Its bidi handling also
treats ActualText as logical text. The measured DLL remains the authority for
this gate; the [upstream text-page implementation](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/core/fpdftext/cpdf_textpage.cpp)
explains the mechanism, but is a moving branch rather than the exact binary.

Native layout clusters must remain the authority for editable overlay selection.
That alone does not solve external PDF selection or Folio's ordinary PDFium
search/highlight geometry after flattening and reopening a PDF without editable
metadata. Whole-span extraction success cannot stand in for that test.

## Independent inspection of native output

`src-tauri/examples/shaped-text-probe.rs` produced 36 native cases: nine samples
at 0/90/180/270 degrees, each with original and engine-exported PDFs. These use
**HarfRust 0.13.3, tracking HarfBuzz 14.3.1**; the earlier Python experiment uses
**HarfBuzz 14.2.1** and a different cluster level. Do not conflate their strings.

Run `python scripts/inspect-shaped-native.py --check-invariants` after the native
example. The independent report is `artifacts/shaped-text/native-independent.json`;
its input is `artifacts/shaped-text/native`. It reads all 72 PDFs, retains raw
MuPDF/pypdf strings and raster hashes, hashes embedded FontFile2 streams, and
compares original PDF CIDs/text matrices to native layout glyph IDs/positions.
The optional exit check tests representation invariants, including preservation
of each reader's raw strings and MuPDF raster through export; known reader failures
remain measured evidence and do not turn into a false test pass or an expected
exception hidden from the report.

Measured results:

- MuPDF exact logical extraction: **24/36**, both before and after export.
- pypdf exact logical extraction: **24/36**, both before and after export.
- MuPDF RGB raster hashes at 2x scale: **36/36 unchanged** after export.
- Both readers' raw strings: **36/36 unchanged** after export.
- Embedded original/exported font bytes: **36/36 match layout font SHA256**.
- Original CID-to-GID maps, text-matrix/draw/glyph counts, and matrix linear
  parts: **36/36 match**. Maximum x/y serialization error: **0.00001 point**.

Extraction failures are the same at every rotation. Below, `\n` is an actual
reader line feed, including interior newlines; nothing is normalized away.

| Native source | MuPDF raw output | pypdf raw output |
| --- | --- | --- |
| `q\u0307\u0323` | `q\u0307\u0323\n` | Entire three-scalar source repeated three times |
| `سلام` | `مالس\n` | Exact |
| `ABC سلام 123 DEF` | `ABC سلام 321 DEF\n` | `DEF` |
| `किताब` | `किताब\nताब\n` | `किकिताताब` |

Ligatures on/off, composed/decomposed accents, and the supplementary sample
extract exactly in both readers. The native PDFium results separately record
logical extraction success and inaccurate whole-span character geometry.
These checks establish failures despite exact intended font/glyph/position
serialization; identical pre/post hashes alone do not prove typography correct.

The actual PDFium setter-API PDF, `artifacts/shaped-text/pdfium-api.pdf`, also
passed independent reading of all four pages. The `ABC` control extracts exactly.
The mark variants render identically in MuPDF. No marked-content variant fixes
pypdf duplication; the common ActualText-parameter variant gives exact MuPDF
mark extraction while individually marked objects duplicate it further. Raw
strings and raster hashes are under `api` in the aggregate report.

### Bounded cluster-span follow-up

The Python research probe additionally emits `logical_clusters`: preserve visual
glyph positions, paint clusters in logical order, wrap each in logical ActualText,
and provide ToUnicode only for single-glyph clusters. This is a fifth experiment,
not a native serializer change. Its full 30-page report is
`artifacts/shaped-text/interop-clusters/results.json`.

This does not clear the gate. MuPDF Arabic and Indic become exact, while PDFium
Arabic becomes `ملاس` and Indic becomes `कि ताब` with an inserted interior
space. Mixed text becomes `ABC 123 ملاس DEF` in PDFium and gains interior
newlines in MuPDF. pypdf exposes raw CID control scalars where ToUnicode was
omitted. Fixing one reader can regress another with unchanged visible positions.

## Native contract and follow-up gate

The shared native result must retain exact immutable font identity, logical
UTF-8/UTF-16 text ranges, visual run order, glyph IDs, advances, both offsets,
grapheme boundaries, and glyph ink. Arabic and Devanagari samples use their
explicit Noto fixtures; mixed Latin/Arabic/digits uses DejaVu Sans without
fallback. DejaVu Sans supplies the two-axis mark fixture; Serif handles ligatures,
accents, and supplementary U+1D434.

Inspect a native output independently with:

```powershell
python scripts/probe-shaped-text-interop.py --python-deps artifacts/complex-text/python --inspect path/to/native.pdf --output artifacts/shaped-text/native-interop
```

The candidate must preserve exact logical extraction in pinned PDFium and
MuPDF, and accurate cluster/selection geometry, before broader UI support.
Record pypdf incompatibilities honestly. Test missing glyphs, explicit control
policy, liga on/off, canonical accents without changing stored text, offsets,
mixed digits, supplementary text, all quarter-turn rotations, and native
save/reopen. Compare actual glyph positions and independently rendered ink,
not only pre/post hashes of the same potentially incorrect representation.

Changing ActualText span granularity, grouping text operators, allocating
cluster-aware ToUnicode, or using a separate semantic representation remain
research options. None is validated by these measurements. A semantic workaround
must not duplicate visible/invisible text, damage selection geometry, or claim
accessibility merely because one extraction API returns the source string.

## Version-matched Unicode conformance sources

The native agent verified these exact data versions in the selected crates:

| Dependency | Unicode data | Primary development fixture sources |
| --- | --- | --- |
| `unicode-bidi 0.3.18` | 16.0.0 | [BidiTest.txt](https://www.unicode.org/Public/16.0.0/ucd/BidiTest.txt), [BidiCharacterTest.txt](https://www.unicode.org/Public/16.0.0/ucd/BidiCharacterTest.txt) |
| `unicode-segmentation 1.13.3` | 17.0.0 | [GraphemeBreakTest.txt](https://www.unicode.org/Public/17.0.0/ucd/auxiliary/GraphemeBreakTest.txt) |
| `unicode-script 0.5.8` | 17.0.0 | [Scripts.txt](https://www.unicode.org/Public/17.0.0/ucd/Scripts.txt), [ScriptExtensions.txt](https://www.unicode.org/Public/17.0.0/ucd/ScriptExtensions.txt) |

The Unicode data versions differ intentionally in the currently selected crate
set. Do not run Unicode 17 bidi expectations against Unicode 16 tables and call
the mismatch a Folio regression, or advertise uniform Unicode 17 support.
The [Unicode 16 ReadMe](https://www.unicode.org/Public/16.0.0/ucd/ReadMe.txt)
identifies that data release; the [Unicode 17 segmentation specification](https://www.unicode.org/reports/tr29/tr29-47.html)
defines the relevant grapheme rules. Download the larger complete bidi datasets
as development artifacts with recorded sizes and SHA256; vendor a small
attributed regression subset only when its selection and expected answers are
explicit. Preserve the complete [Unicode License V3](https://www.unicode.org/license.txt)
alongside redistributed Unicode data. These links alone are not a conformance
claim.

Versioned development fixtures are now in `tests/fixtures/shaped-text/unicode`:
the complete Unicode 17 grapheme test file (126,570 bytes), a deterministic
286-row Unicode 16 BidiCharacterTest sample (24,663 bytes), and the complete
Unicode License V3 (1,995 bytes). The full bidi source has 91,707 test rows /
6,880,649 bytes and is retained only as an artifact. The sample preserves the
source header, first/last 16 test rows, and 256 evenly spaced test-row indices,
deduplicated and sorted. Adjacent `provenance.json` records source/sample SHA256,
selection algorithm, and exact source line numbers. These do not replace the
full BidiTest or BidiCharacterTest suites; native execution results must state
sampled versus complete coverage separately.
