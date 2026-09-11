# Long native semantic PDF: bounded verification

This matrix checks repeated text beyond the former 255-scalar encoding limit.
The native experimental representation reuses a single-byte Type3 code only
when its Unicode scalar, rounded advance, and exact local bounding-box bits
match. There are still at most 255 distinct character definitions per PDF, and
the shaping input remains limited to 4096 Unicode scalars and 16384 UTF-8 bytes.
The visible vector outlines and every occurrence's `TJ` position remain separate
from the reused blank character definitions.

## Reproduce

Generate fresh artifacts with the `shaped-text` feature and
`shaped-text-probe --semantic-long`, passing a new output directory. Then run:

```powershell
python -B scripts/inspect-semantic-native.py --long-text --input artifacts/shaped-text/semantic-long-native-verified --output artifacts/shaped-text/semantic-long-native-verified-independent.json --summary docs/semantic-long-native-verification.json --check-invariants
python -B -m unittest discover -s tests -p semantic_native_inspector_test.py
```

The explicit summary path preserves the historical
[`32-case report`](semantic-native-verification.json). The long inspector refuses
to overwrite that historical summary. It independently reads both original and
engine-exported PDFs with MuPDF and pypdf, verifies retained font bytes against
the native layout SHA256, and checks each emitted text byte against the
single-byte CMap, Encoding, Widths, and blank `d1` CharProcs. Decoded visual-order
scalars must match the native layout, while all three readers must return the
exact logical source. A surrogate-pair CMap destination is one Unicode scalar.

The seven samples run at 0, 90, 180, and 270 degrees, for 28 positive cases:

| Sample | Source scalars | Font size |
| --- | ---: | ---: |
| `office` repeated 64 times with spaces, ligatures on | 447 | 24 pt |
| Same text, ligatures off | 447 | 24 pt |
| `q` + U+0307 + U+0323 repeated 80 times with spaces | 319 | 24 pt |
| Arabic `سلام` repeated 80 times without separators | 320 | 24 pt |
| Devanagari `किताब` repeated 64 times with spaces | 383 | 24 pt |
| `A` + U+1D434 + `B` repeated 128 times | 384 | 24 pt |
| `i` repeated 4096 times | 4096 | 4 pt |

Space-separated samples have no trailing space. Four mixed-direction requests
and four RTL requests containing word boundaries are separate required
refusals, one of each per rotation.

## Measured results

The [final long report](semantic-long-native-verification.json) records
**28/28 full invariant passes**, plus all eight required refusals. Its source
strings, reader output before/after export, hashes, and definition counts are
retained without normalization beyond ignoring final reader CR/LF terminators.

| Check | Result |
| --- | --- |
| Exact logical Unicode in PDFium, MuPDF, and pypdf, before and after export | 28/28 |
| Original font hash, single-byte code coverage, Unicode mapping, Widths, blank CharProcs, one text object and one `TJ` | 28/28 original and exported |
| Raw text, PDFium/MuPDF geometry, and each reader's raster preserved after export | 28/28 |
| PDFium source-cluster geometry within 0.06 pt | 28/28 |
| Mixed-direction refusal | 4/4 |
| RTL word-boundary refusal | 4/4 |

Distinct definitions per font are 7 for ligatures on, 6 for ligatures off, 4 for
marks, 24 for uninterrupted Arabic, 33 for Devanagari, 3 for supplementary text, and 1 for the
4096-scalar sample; all four rotations agree. The 4096-scalar sample's maximum
PDFium cluster-edge difference is 0.001086 pt, rounding upward. The largest
difference across the matrix is 0.012236 pt, rounding upward. MuPDF's maximum
cluster-edge difference remains 12.600459 pt, rounding upward; that reader's
cluster geometry is not claimed equivalent.

The new serializer also passes a fresh
[32-case short regression matrix](semantic-short-reused-verification.json),
including its four mixed refusals. That separate report preserves the original
historical evidence.

Long-matrix PDFium previews target a 2000-pixel maximum dimension; the engine
records the selected `renderWidth` per case and enforces a minimum render width
of 64 pixels. The narrowest rotated samples therefore reach 2716 pixels in
height. This keeps narrow rotated pages from producing extremely tall raster
buffers. MuPDF independently renders each
PDF at 2x native page size. Raster checks compare each candidate before and
after export using the same reader and scale; they do not compare different
readers or filled outlines against hinted TrueType text.

## RTL boundary refusal

The [initial spaced-Arabic report](semantic-long-rtl-words-negative.json)
preserves **24/28 invariant passes** and four failures from an earlier matrix
containing 64 space-separated copies of `سلام` (319 scalars, 30 definitions).
For those cases, PDFium's character geometry follows the leftmost word first
while native logical source positions start at the rightmost word. At zero
degrees the first source `س` expects x=3290.012 pt, but its engine geometry is
x=73.992004 pt. The maximum discrepancy is 3216.032 pt. Each word's letters
remain in logical order, and identical repeated words conceal the word-order
problem in an exact-text-only comparison. The inspector preserves the
source-indexed comparison and fails; it does not remap equal words or widen
the tolerance.

A distinct-word native regression confirmed actual copy corruption as well:
source `سلام عالم` extracted as `عالم سلام` before the guard. The regression
now requires refusal at each rotation for ASCII space, NBSP, Arabic comma
and ZWNJ boundaries.

The repeated-word failure led to a conservative semantic-output guard: RTL
inputs must contain only strong RTL characters (Unicode bidi class R or AL)
and nonspacing marks (NSM). Neutral boundaries and joiners are refused before
serialization. Native shaping itself is unchanged. This is a bounded reader
policy, not a statement that all punctuation is intrinsically unsupported.

A [separate CMap-only probe](semantic-rtl-separator-probe.json) changed the space
mapping in copies of the failing PDF while keeping positions fixed. ASCII
space, NBSP, Arabic comma, ASCII comma, hyphen, and ZWNJ all retained the wrong
word order. Arabic semicolon, Arabic question mark, and an Arabic-letter control
instead started at the rightmost occurrence. This establishes that a
whitespace-only refusal would miss known failures. The substitutions do not
reshape the source and do not establish correct glyph geometry for substituted
text. Reproduce against the preserved pre-refusal PDF with:

```powershell
python -B scripts/probe-semantic-rtl-separators.py
```

Its default input is the single checked-in
[negative PDF](../tests/fixtures/shaped-text/semantic-negative/rtl-word-order.pdf).
The [fixture provenance](../tests/fixtures/shaped-text/semantic-negative/README.md)
records generation parameters, PDF/font hashes, and the existing Noto OFL
notice. The current serializer refuses that PDF's source input; keeping the
original PDF makes the failure reproducible without disabling the new guard.

## Scope

The 0.06-point PDFium cluster-bound tolerance is a fixture measurement gate,
including the 4-point maximum-length case, not a scale-independent guarantee.
Whitespace retains the engine's loose font bounds and is recorded separately.
Repeated scalar boxes inside a cluster represent coarse selection, not distinct
caret positions. MuPDF cluster geometry is measured separately and remains
different; passing Unicode and raster preservation does not establish equivalent
cluster selection across readers.

This experiment does not establish arbitrary long text, mixed bidi, full script
support, accessibility behavior, or production editor readiness. Exact geometry
differences can consume separate definitions even for the same Unicode scalar;
a 256th distinct definition is explicitly refused. Original font bytes remain
an unused provenance resource, and visible text is still outlined.

## Regression verification

Checks completed on 2026-09-11:

| Check | Result |
| --- | --- |
| Full Rust library/integration suite with `shaped-text` | 118 passed, 3 existing ignored |
| Full Unicode 16 bidi character corpus / Unicode 17 grapheme corpus | 91,707 / 766 rows passed |
| Semantic serializer integration tests within that suite | 7 passed |
| Independent inspector positive/negative tests | 9 passed |
| Original 32 cases versus the previous serializer's artifacts | Identical PDFium text, geometry and PNG hashes for all 32 |
| Long native reader inspection | 28 passed, 8 required refusals |
| Separator probe against the retained negative PDF | Reproduced all nine recorded cases |
| Rust formatting / Git whitespace checks | Passed |

The serializer tests cover the 4,096-scalar copy/selection limit, rejection at
4,097 scalars, the 255/256-distinct-definition boundary, and RTL boundary
refusal, alongside existing outline/font/export checks. The inspector rejects
incorrect Unicode mappings, missing/duplicate codes, missing character programs,
width mismatches and invalid surrogate mappings instead of trusting equal
dictionary counts.

```powershell
$env:FOLIO_FULL_BIDI_TEST = '<path to BidiCharacterTest-16.0.0.txt>'
cargo test --offline --locked --manifest-path src-tauri/Cargo.toml --features shaped-text --tests --lib
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

The frontend and default engine are unchanged in this increment. No new
frontend suite, packaged desktop smoke or manual Foxit/Acrobat copy/search
check was run. The complex-text feature remains disabled in application builds,
and v0.9.0 remains the latest release.
