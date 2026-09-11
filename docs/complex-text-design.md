# Complex text and positioned PDF text: investigation

Research date: 2026-09-11. Scope: [issue #13](https://github.com/willherr72/folio/issues/13), investigated from `a12a4c1` alongside the bounded embedded-font work in #12. This document proposes implementation stages. **The application does not gain shaping, bidi, Indic, Arabic, or paragraph editing from this investigation.**

## Decision

Proceed with a shared logical-text and shaped-glyph model before widening the font registry's accepted character ranges. Current PDFium has useful primitives for emitting positioned glyphs; extraction, selection, and matching browser preview remain separate engineering work. Start with explicitly added single-line text, establish reader interoperability, then extend existing text and explicit text areas in separately reviewed stages.

The local probe establishes feasibility of shaping the sample strings and rendering a small positioned PDF. It also demonstrates a real failure of a tempting Unicode mapping: assigning a cluster's complete text to every glyph duplicates combining marks on copy. `/ActualText` fixes two tested readers but not the third. This result is an implementation gate, not evidence that a production encoding has been solved.

## Current constraints

At the investigation baseline, `src-tauri/src/fonts.rs` validates static TrueType programs and deliberately limits custom text to unshaped BMP ranges. `src-tauri/src/custom_font_pdf.rs` makes CIDs equal Unicode BMP values, maps those CIDs through the font's nominal cmap, and supplies nominal advances plus an identity `ToUnicode`. That representation cannot express one Unicode character with different contextual glyphs, a ligature covering several characters, or reordered glyph clusters.

`src-tauri/src/text_edit.rs` protects original bytes, restricts existing-run transformations/spacing, and checks no-op and saved/reopened text, geometry, and pixels. Issue #12 adds verified reuse/substitution for a bounded horizontal Latin run; it does not remove the need for those proofs. See the [current font roadmap](font-support-roadmap.md) and [embedded-font design](superpowers/specs/2026-09-11-embedded-font-editing-design.md).

Keep the existing resource limits and ownership model: at most 64 registered fonts, 16 MiB per font and 128 MiB total, immutable font bytes referenced by active documents, history, and recovery. A new layout object must participate in that ownership instead of holding an untracked native pointer or independently installed browser font.

## What the libraries provide

HarfBuzz shapes a run with one font, script, language, and direction into glyph IDs, advances, and offsets. The caller must perform bidi processing, split incompatible runs, and choose line breaks. Adding HarfBuzz alone does not provide paragraph layout. [HarfBuzz responsibilities](https://harfbuzz.github.io/what-harfbuzz-doesnt-do.html)

Unicode remains in logical order. Resolve paragraph embedding levels, shape directional runs, and apply the bidi algorithm's line-specific visual reordering after line boundaries are known. Do not reverse an Arabic string before shaping, or reverse already shaped RTL glyphs a second time. A production implementation needs explicit isolates, bracket handling, controls, and a pinned Unicode-data version. The probe's legacy Python bidi path is limited to its simple mixed-direction sample. [Unicode Bidirectional Algorithm](https://www.unicode.org/reports/tr9/)

HarfBuzz cluster identifiers retain the relation between input indices and output glyphs through substitutions and reordering. The probe supplies scalar indices and requests `MONOTONE_CHARACTERS` (level 1), then records scalar and UTF-16 ranges. One cluster can cover several characters or several glyphs. Cluster boundaries are not interchangeable with editing grapheme boundaries. [HarfBuzz clusters](https://harfbuzz.github.io/working-with-harfbuzz-clusters.html), [Unicode text segmentation](https://www.unicode.org/reports/tr29/)

### Bundled PDFium, not an assumed upgrade

The repository pins `pdfium-render = 0.9.4` with the `pdfium_7881` binding feature. `scripts/fetch-pdfium.ps1` pins the binary release `chromium/8044`. The probe loaded the actual Windows DLL and verified the symbols below. Its SHA-256 was `04100c03e41cac1f979e36e5e26fb860bcb5a7461f53830d3c098716624a27a9`, matching local provenance. The binding version and binary revision are different identifiers; neither should be described as a HarfBuzz version.

| API | Relevant capability and limit |
| --- | --- |
| `FPDFText_SetText` | Unicode-to-font character-code assignment; not a shaping contract. |
| `FPDFText_SetCharcodes` | Takes PDF character codes. HarfBuzz glyph IDs are valid inputs only when the chosen encoding/CID mapping makes them so. |
| `FPDFText_LoadCidType2Font` | Accepts caller-provided `ToUnicode` and `CIDToGIDMap` with font bytes. |
| `FPDFText_SetPositions` | One position in points per character after the first; first position is implicitly zero. Count must be N−1 and N must exceed one. Axis follows the text's writing mode. |
| `FPDFPageObj_CreateTextObj`, `FPDFPageObj_SetMatrix` | Permit separate objects/origins for glyph groups with different offsets. |
| `FPDFFormObj_CountObjects`, `FPDFFormObj_GetObject` | Traverse form contents; do not themselves establish safe ownership for editing one occurrence of a shared form. |
| `FPDFFont_GetFontData` | Accesses font bytes, including substitution data for a nonembedded font; byte availability alone does not prove source-font identity. |

This table is based on the installed crate's `include/pdfium_7881/fpdf_edit.h` and `src/bindings.rs`, checked against the DLL exports. All listed exports were present. The probe **does not call the setter APIs**, so this is availability evidence rather than a native integration test. It creates the research PDF directly and uses PDFium for extraction.

Primary upstream references explain [the public editing API](https://pdfium.googlesource.com/pdfium/+/main/public/fpdf_edit.h), [the addition of caller-controlled CID/Unicode maps](https://pdfium.googlesource.com/pdfium/+/7c7a6087e09e1a344984a6d0c5fbc2af36eca7ea), and [text assignment implementation](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/fpdfsdk/fpdf_edittext.cpp). Upstream's [editing tests](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/fpdfsdk/fpdf_edit_embeddertest.cpp) include Bengali positions calculated with HarfBuzz, separate objects for different vertical offsets, and save/reopen raster checks. These upstream examples support feasibility; they do not replace tests against Folio's pinned binary.

`SetPositions` can represent accumulated advance changes and same-axis offsets. General mark placement needs both coordinates, so one text object per entire line is insufficient. Candidate implementations are grouped text objects with compatible baselines, or explicit `Tm`/`Tj` operations in Folio's appearance streams. Benchmark the object count and generated-content behavior before choosing. PDFium text objects and portable appearance streams must use the same layout and font maps.

## Reproducible local evidence

The research harness is [scripts/probe-complex-text.py](../scripts/probe-complex-text.py). Dependencies are development-only under ignored `artifacts/complex-text/python`; no Cargo or production frontend dependency changed.

From the repository root:

```powershell
python -m pip install --target artifacts/complex-text/python uharfbuzz==0.55.0 python-bidi==0.6.11 fonttools==4.62.1 pypdf==6.14.2 pymupdf==1.27.1
python scripts/probe-complex-text.py
```

Measured environment: Python 3.14.0; uharfbuzz 0.55.0 using HarfBuzz 14.2.1; python-bidi 0.6.11; fontTools 4.62.1; pypdf 6.14.2; PyMuPDF 1.27.1. The first two packages were installed into the isolated research directory; the remaining recorded versions were already available. Reproduction above pins all five. The harness supports `--output`, `--arabic-font`, `--indic-font`, `--indic-index`, and `--pdfium`; absent optional fonts/DLL are recorded as skipped, not successful coverage.

Outputs under `artifacts/complex-text/`:

- `results.json`: versions, font paths/face indices/hashes, glyph IDs, advances/offsets, cluster ranges, bidi levels/run order, DLL exports, raw reader results, and exact-match booleans.
- `cluster-extraction.pdf`: six pages containing Latin ligature, decomposed-accent, and positioned-mark experiments, each with and without replacement text.
- `page-*.png`: MuPDF renders at 2× resolution. The harness asserts that adding replacement text does not change the paired raster pixels.

Artifacts are intentionally ignored by git and can be regenerated. The PDF embeds only the repository's licensed DejaVu Serif fixture; its adjacent `LICENSE_DEJAVU.txt` applies. Tahoma and Nirmala are read from Windows for shaping only and are not copied or embedded in these outputs. Nirmala is a TTC face used by the research binding; this does not add TTC import support to Folio. `fsType` is recorded as evidence, not treated as a complete distribution license.

| Font input | Face | SHA-256 |
| --- | --- | --- |
| Repository DejaVu Serif | 0 | `107244956e9962b9e96faccdc551825e0ae0898ae13737133e1b921a2fd35ffa` |
| Windows Tahoma | 0 | `9af03d4ad44a3b413d92f7de48b94aa7cc8a1471a75d498406eae837f62ee1d1` |
| Windows Nirmala UI collection | 0 | `ad02cdfc06e144ac45f318e8e5a64cbe04c7479d4beb91d25f5a319a466b1767` |

| Sample | Measured result | Design implication |
| --- | --- | --- |
| `office`, DejaVu Serif | 6 scalars → 5 glyphs with `liga`; 6 without. `ff` becomes glyph 3311. | A glyph may represent several source characters. |
| `café` / `cafe\u0301` | Both produce 4 glyphs, from 4 / 5 scalars respectively. | Preserve original Unicode; do not infer it from displayed glyph count or silently normalize input. |
| `q\u0307\u0323` | 3 glyphs; marks share scalar cluster [1,3), have zero advance, and nonzero offsets including y=−430 font units. | Codepoint advance sums and one-axis positioning do not fully describe the visible bounds. |
| `سلام`, Tahoma | 4 scalars → 3 contextual/ligature glyphs; clusters descend [3,4), [1,3), [0,1). | Context and logical-to-visual mapping matter even for a short word. |
| `ABC سلام 123 DEF`, Tahoma | Logical run levels 0/1/2/0; visual run order [0,2,1,3]. | Digits remain LTR inside the mixed paragraph; reversing the whole text is wrong. |
| `किताब`, Nirmala UI | 5 scalars → 5 glyphs; first two glyphs share [0,2), with the i-matra glyph before the consonant. | Equal input/output counts do not establish a one-to-one map. |

All measured cases and mixed-direction subruns had zero missing glyphs. These are small samples, not language conformance or a native-speaker typography review. Arabic/Indic/bidi were **shape-only** probes: their export, screen selection, IME behavior, and accessibility were not exercised here.

### Copy/extraction negative control

The PDF experiment allocates a CID for each glyph occurrence, maps it to the correct shaped glyph ID, positions it explicitly, and naively maps every CID to its complete input cluster. A second page variant adds one logical-run `/ActualText` span. This deliberately tests a flawed simple mapping; it is not proposed production code.

Exact comparisons below strip only trailing CR/LF added by reader APIs. They do not normalize accents, remove interior newlines, reorder RTL text, or deduplicate characters.

| Content / mapping | PDFium chromium/8044 | MuPDF via PyMuPDF 1.27.1 | pypdf 6.14.2 |
| --- | --- | --- | --- |
| `office`, cluster map | Exact | Exact | Exact |
| `cafe\u0301`, cluster map | Exact decomposed source | Exact decomposed source | Exact decomposed source |
| `q\u0307\u0323`, cluster map | Duplicate marks plus interior CR/LF | Duplicate marks | Duplicate marks |
| Same marks plus `/ActualText` | Exact | Exact | Duplicate marks |

Both variants of the ligature and decomposed-accent cases extracted exactly in all three readers. The marks looked identical between variants. Thus extraction failure can survive a successful raster comparison, and `/ActualText` is not a universal compatibility fix in the tested stack.

PDF defines replacement text for content whose glyph representation does not directly express the intended characters. [PDF 1.7 replacement text, §10.8.3](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.7old.pdf) Adobe's own glyph-run API also anticipates cases requiring replacement spans beyond a `ToUnicode` map. [Adobe PDEFont API](https://opensource.adobe.com/dc-acrobat-sdk-docs/acrobatsdk/apireference/PDFEdit_Layer/PDEFont.html) Neither source implies that every extractor implements this consistently.

## Proposed architecture

### One layout shared by preview, export, and interaction

Preserve the user's logical Unicode string exactly. Store explicit mappings among UTF-8 byte offsets, Unicode scalar indices, and JavaScript UTF-16 offsets. Store paragraph direction and language as editable properties; automatic inference must be reproducible. Do not rewrite the document string to a visual-order string or normalize accents as a side effect of shaping.

A versioned shaped run should contain font content ID and face selection, size, features/variation coordinates if eventually supported, script/language/direction, bidi level, logical range, glyph IDs, x/y advances, x/y offsets, glyph ink bounds, and input cluster ranges. Preserve grapheme boundaries separately for caret/delete operations. A logical search match maps to one or more visual rectangles through those ranges; an RTL or multi-line match need not be one rectangle. A ligature may need internal caret positions or a documented whole-cluster selection policy, rather than fabricated per-character advances.

Shape the exact validated font bytes. Cache by text, font digest, complete shaping options, and layout-engine/Unicode versions. Pin ownership while native jobs, preview, export, history, or recovery reference the result; cancel or discard stale completions using document/layout generations. Bound output glyph count, processing time, layout-cache bytes, PDF object count, and decoded font resource size. Bidi controls and zero-width characters need an explicit editing policy, not blanket deletion. Missing glyphs or an unsupported font/script should produce a precise failure instead of unrecorded fallback.

Recommended first integration: one native shaping result feeds portable PDF appearance generation and a preview raster or glyph-path layer; a logical text input remains responsible for IME/accessibility. This keeps one shaping implementation and avoids claiming that arbitrary CSS text matches a separately shaped native run. A shared HarfBuzz WASM build is an alternative if measured typing latency makes native preview unsuitable, but it adds another binary/version/lifetime surface. Browser FontFace with the same bytes is useful for loading, not proof of identical layout across browser/native versions and features. Test IME composition and accessible logical text before choosing the final editing surface.

### PDF encoding and Unicode

Allocate CIDs independently of Unicode codepoints. A font-resource mapping must distinguish glyph ID **and its semantic role**: the same glyph can represent different source sequences, and one sequence can yield several glyphs. Reserve CID zero and split resources before exhausting the 16-bit CID space. Emit widths from the actual glyph metrics; use shaped positions for placement. `ToUnicode` destinations can contain UTF-16 sequences, including surrogate pairs, even though CIDs are 16-bit.

Do not assign a whole cluster to every glyph. Evaluate per-occurrence semantic allocation and minimal cluster replacement spans, preserving original logical order and the original string in validated Folio metadata. Mapping only the first glyph and leaving other glyphs unmapped is not an accepted shortcut: readers can synthesize fallback text. Likewise, invisible duplicate logical text can create duplicate selection, accessibility, and search results. Any candidate must pass raw extraction and character-geometry tests in the target readers. PDF's [character-code/Unicode mapping model](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.6.pdf) is the basis; the precise multi-glyph-cluster strategy remains unresolved by this spike.

Folio copy/search should operate on its logical string plus cluster geometry for owned text, never reconstructed glyph names. That improves in-app behavior but does not excuse broken external extraction. On reopen, accept owned metadata only after verifying its font resources, content, glyph positions, and mapping against bounded regenerated expectations, as #11 already does for simpler appearances. Foreign PDFs without trustworthy Unicode must stay viewable; ambiguous replacement must remain rejected. Define a schema migration for old unshaped overlays rather than silently changing their visible layout when HarfBuzz is introduced.

### Positioned source runs and form ownership

Keep editing original source and adding a new text box as distinct operations. First expand explicit spacing/affine transforms for a single verified source run. Preserve baseline, writing mode, glyph positioning, and complete inherited text state; apply collision/crop checks in page coordinates. The current page-source collision scope must be stated accurately; separate annotations/overlays require their own policy.

Next allow an explicitly selected adjacent run group with a verified logical mapping. Geometric proximity alone does not establish reading order, common font semantics, or a paragraph. Reject interleaved graphics, ambiguous glyph encoding, unsupported clip/blend states, and uncertain ownership until each has a tested representation. Keep a no-op save/reopen proof and compare all nonselected content, including pixels outside the authorized edit region.

For forms, identify the selected occurrence by page and invocation path, not just the shared XObject ID. Editing one placement requires cloning the affected shared form/resource chain and repointing that occurrence before mutation. Preserve accumulated matrices, clipping and inherited resources; detect cycles and cap traversal depth/decoded bytes/object visits. Never insert a borrowed child object into another page and assume that transfers safe ownership. A dedicated fixture must place the same form twice on one page and on another page, then prove only the selected occurrence changed. Separate tests must cover shared nested forms and recovery after the copy.

### Explicit areas before paragraph reflow

Start wrapping with a user-created rectangle that stores width, height, inset, rotation, paragraph direction, alignment, line spacing, and overflow policy. Initial policy: show overflow and block commit/export of new overflow; no silent shrinking, clipping, or continuation onto another page. Decide that behavior in product review before implementation.

Use Unicode line-break opportunities, shaped widths and font extents to choose lines, then reshape at chosen boundaries and apply bidi reordering per line. Respect grapheme and unsafe-to-break information; a break opportunity is not itself an instruction to break. [Unicode line breaking](https://www.unicode.org/reports/tr14/) Hard newlines stay in the logical text; soft wrapping must not add copy/search characters. Cache invalidation includes rectangle dimensions and layout settings. Existing PDF paragraphs, table reflow, hyphenation dictionaries, justification, vertical writing, OCR, and secure redaction remain outside this first area model.

## Proposed follow-up issues

Implementation is tracked in [#14](https://github.com/willherr72/folio/issues/14), [#15](https://github.com/willherr72/folio/issues/15), and [#16](https://github.com/willherr72/folio/issues/16). These stages are planned work. [#13](https://github.com/willherr72/folio/issues/13) remains open as the umbrella for the investigation and implementation gates.

### A. Add shaped single-line text with bidi, cluster geometry, and portable Unicode

**Problem:** Custom text currently assumes nominal BMP character-to-glyph mappings. That breaks contextual scripts, ligatures, and combining marks, and cannot keep preview, PDF output, copy, and selection in agreement.

**Scope:** Introduce a versioned logical-text/shaped-run model for explicitly added single-line text using validated immutable font resources. Select and pin shaping/bidi libraries and Unicode data; use one layout for preview and portable PDF output. Add allocated CID/glyph/Unicode maps and a tested strategy for multi-glyph clusters. Keep existing unshaped documents visually compatible through an explicit schema path.

**Acceptance criteria:**

- Latin `ff`/`fi` ligatures on/off, composed/decomposed accents, zero-advance marks, Arabic joining, mixed Arabic/LTR digits and punctuation, one redistributable Indic fixture, supplementary Unicode with a covered glyph, and unsupported-glyph failures.
- Exact logical Unicode after save/reopen and extraction in pinned PDFium plus at least one independent reader; record pypdf behavior and resolve or explicitly scope incompatibilities before script support is advertised. Add an Acrobat/another real-reader manual copy/search check where available; do not infer it from MuPDF.
- Pixel/geometry agreement between editing preview and saved appearance at 0/90/180/270° and multiple zooms; no shifted marks or visual-order copy.
- Selection/search map logical matches to cluster bounds; test ligature interiors, grapheme deletion, RTL boundaries, and IME composition without premature replacement.
- Undo/redo, recovery, duplicate tabs, export cancellation and late shaping/font responses preserve exact font ownership and discard stale output.
- Bounded hostile-font/layout/resource tests; no untracked font fallback. Run version-matched Unicode bidi/grapheme conformance data for the supported implementation.

**Dependencies:** #11 font resources; issue #13 evidence and agreed reader compatibility policy. #12 remains the separate bounded existing-run path.

**Exclusions:** Paragraph reflow, inferred text areas, existing nested-form editing, variable/color fonts, universal script coverage. Implement a native/API serialization spike first; widen supported characters only with passing end-to-end cases.

### B. Edit verified positioned runs and isolated form occurrences

**Problem:** PDF producers can space individual glyphs, split one visible line across objects, or reuse a text-containing form in several placements. String replacement can lose positions or alter other occurrences.

**Scope:** Stage B1 handles explicit spacing and affine transforms for one verified run; B2 adds explicitly selected compatible adjacent groups; B3 isolates a chosen nested-form occurrence through resource copying before editing. Each stage keeps unsupported cases rejected with precise reasons and can ship independently after its tests pass.

**Acceptance criteria:**

- Fixtures for `TJ`, character/word spacing, rise, horizontal scale, transformed baselines, negative spacing and split streams. Preserve selected geometry on a no-op save/reopen and reject uncertain Unicode mappings.
- Stable selection target through page/object/invocation identity; no accidental merge based only on proximity or font name.
- Shared form used twice on one page and on another page: editing one occurrence preserves the other two exactly. Repeat with two nested levels, inherited resources and different transforms.
- Clip/blend/transparency cases either preserve semantics with evidence or fail before mutation. Cyclic/deep/shared resource graphs have bounded traversal and decompression.
- Original document bytes, neighboring content, object ordering, page crop, undo/redo and recovery remain protected. Include PDFium and independent-reader saved-output checks.
- Record memory/object growth for repeated edits and cancellation; release copied resources when history/documents no longer own them.

**Dependencies:** #12 for verified original-font reuse/substitution. A is required before extending these operations to shaped text; B1's unshaped subset can proceed separately.

**Exclusions:** Automatic paragraph reconstruction, OCR, arbitrary tagged-PDF restructuring, or modifying all shared occurrences by default.

### C. Add explicit bounded text areas with wrapping and overflow feedback

**Problem:** Added text is positioned by lines without a persistent layout-area contract. Wrapping cannot safely be inferred from surrounding PDF marks or delegated independently to browser and export renderers.

**Scope:** User-created text areas with stored dimensions, inset, direction, alignment, line spacing and reviewed overflow policy. Choose line breaks from logical text and shaped measurements, reshape boundaries, and produce the same lines in preview/export. Keep source PDF content outside the area untouched.

**Acceptance criteria:**

- Stable line breaks before/after save/reopen/recovery for Latin, accents, Arabic with digits, and the supported Indic fixture. Test narrow boxes, long unbreakable clusters, explicit newlines, nonbreaking spaces, and bidi controls.
- Resize, rotate and change fonts at multiple zooms without splitting graphemes, losing characters or silently scaling the font. Per-line visual reorder matches logical copy/search.
- Visible overflow state and deterministic commit/export behavior approved in design; no silent clip or hidden missing text.
- Selection and keyboard movement cross soft/hard line boundaries correctly; soft wraps add no copied characters. Preserve IME composition and accessible logical text.
- Undo/redo and crash recovery restore box geometry, settings, fonts and logical content. Native and independent-reader raster/extraction tests agree with preview.
- Bound line/glyph counts and layout work; cancellation and shrinking/removing areas release unused layout resources.

**Dependencies:** A's shared shaping, cluster geometry and serialization contract. Uses B only if a later separately designed operation converts selected existing content into an owned area.

**Exclusions:** Reflow of existing PDF paragraphs, automatic multi-column flow, page continuation, tables, hyphenation dictionaries, justification, OCR and redaction.

## Verification and open limits

The harness ran successfully on 2026-09-11 with seven standalone shaping cases plus the mixed-direction run probe. It asserted no missing glyphs, ligature substitution, nonzero mark offsets, valid cluster ranges, expected mixed-run order, and identical paired MuPDF raster pixels. It measured all 18 reader/page combinations and retained the intentional extraction failures in JSON. A green harness exit means its setup assertions held; it does not mean all extraction candidates passed.

Unverified here: native setter/create/save integration, Arabic/Indic PDF export, real browser preview and editing latency, bidi/segmentation conformance, broad language typography, Acrobat copy/search, cluster character boxes in PDFium, screen-reader behavior, IME handling, wrapping, forms, undo/recovery with shaped data, and performance/resource limits. The proposed issues make these future acceptance work explicit. No production support, performance promise, or complete word-processor behavior follows from this research.
