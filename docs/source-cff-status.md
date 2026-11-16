# Existing CFF source-font editing — development status

## CFF ligatures — verified source candidate

The newer source component now retains verified multi-letter glyph mappings such
as fi and ffi in raw Type1C, CIDFontType0C and static OpenType CFF1 fonts. It uses
the same bounded source-glyph model as existing CID TrueType ligatures. The PDF
must supply the complete ToUnicode sequence; Folio does not infer missing text or
silently substitute a font. Sequences are bounded to 2–16 unshaped Latin scalars,
without whitespace. Code/GID, glyph matrix, positive ink and advances must agree;
known conflicting Unicode aliases or glyph names remain refusals.

The native source checks are complete: 1,759 library passes/11 explicit skips
under the recorded Folio-opt0 correctness profile, 227 ordinary optimized
integration passes/3 skips across 24 affected targets, and 21 parser contracts
plus 190 upstream parser tests. Fresh frontend passed 1,768 tests in 248 files.
The optimized transaction matrix covers 48 authored font/route cases with 96
saved outputs checked independently for whole-page pixels, text/order, physical
glyph codes and font programs. Seven actual corruption controls were rejected.
Font width comparison uses exact native f32 values; decimal serialization can
change spelling without changing those values. It does not discard width checks
or expand an output-derived mask.

The normal optimized diagnostic GUI/CLI build and actual compiled acceptance
passed. The CLI made 201 calls, including four ligature-only subset cases that
retain whole ffi glyphs but refuse unavailable scalar splits. Four MCP sessions
made 72 requests, with Apply/re-edit, stale-selection and missing-glyph checks.
Four owned Windows workflows cover source-font edits, local/document Undo/Redo,
automatic paragraph fitting, shared-form paragraph promotion, save/reopen and
draft recovery. All 72 saved runtime PDFs passed the independent font/code/text
and whole-page-pixel audit. Every owned GUI loaded the candidate renderer and
stopped. Final scoped review found a non-painting-glyph admission defect. The
correction now requires nonzero contour-area evidence, with a scaled rounding
guard, during the same bounded charstring walk. All 36 move-only, diagonal and
retraced-curve source cases are excluded while ordinary scalars and spaces remain
eligible. Fresh library, parser, optimized integration, frontend, normal build
and compiled CLI/MCP/Windows checks passed after the correction. Unusual genuine
contours with zero or numerically uncertain net area can remain unsupported.
This is authored-fixture development acceptance. Physical keyboard/IME, clipboard,
broader producers/readers/DPI, printing and sustained-resource acceptance remain
outside this component.

This candidate is newer than the unchanged 0.54 portable; the selected launcher
and all prior packages remain unchanged. It is not a portable integration or
public-release/parity claim. The source and records are under
[the CFF ligature plan](superpowers/plans/2026-11-15-cff-source-ligatures.md) and
[its ledger](../.superpowers/sdd/2026-11-15-cff-source-ligatures/progress.md).

The implementation reuses verified CFF fonts already embedded in a PDF. It was
qualified in the local 0.46 portable and remains included in selected 0.50
and the separate reviewed 0.52 desktop/CLI package. The corrected CFF source acceptance passed 2,784 native tests;
later portable integrations explicitly reuse those results with their original
profiles. The selected package also passed fresh extracted editing and independent
saved-output checks, standalone integrity, relocation and archive-only checks.
See the [historical 0.49 completion](../artifacts/portable-049-20261112/component-completion.json)
for that integration's scope, and the [current build status](development-build-status.md)
for the selected and separate candidates. This is local development qualification, not a
public release or universal CFF compatibility claim.

Supported source formats are bounded raw Type1C and CIDFontType0C programs and
static OpenType CFF1 wrappers. The shared proof checks original font bytes,
character codes, Unicode mappings, CID/GID relationships, glyph matrices and
fractional advances before permitting an edit. It feeds the existing single-run,
group, uniform/styled paragraph and reusable-object transactions, with the same
unshaped character policy and original-glyph coverage requirements. Source
resources, surrounding content and occurrence ownership remain protected by
the existing publication checks. This does not enable arbitrary CFF font import.

The final scoped review identified two source-proof gaps and an independent
tag-ownership audit gap. Regression tests reproduced malformed post-width
instructions, post-width work expansion and conflicting outline-table acceptance.
The correction validates the full reachable glyph program within the existing
budget, checks termination, preserves valid zero-ink spaces and fractional
widths, and refuses concurrent raw CFF2 or glyf tables. The independent tag audit
now compares ordered Unicode characters, original font identities and owners;
it rejects lost, gained and swapped ownership even when pixels and extracted
text are unchanged.

The corrected complete native run passed 2,784 tests with no failures and
32 explicit skips. It includes all 143 integration/bin targets using ordinary
optimized release builds; the library used release dependencies with Folio
opt-level 0 for correctness. Independent checks passed 72 native CFF transaction
cases, 32 saved tag-ownership stages across eight cases, and 16 explicit font
substitution outputs. The 21 parser contract and 187 upstream parser tests also
passed. Fresh frontend validation passed all 1,528 tests in 228 files with the
unchanged 5,000 ms timeout and two workers. These results do not establish
portable integration or commercial parity.

Fresh normal optimized GUI/custom-protocol and CLI/default builds passed with
all 26 unchanged renderer resources. Ten real Windows desktop workflows and ten
MCP sessions plus 130 CLI calls passed, including draft retention, Cancel,
Undo/Redo, paragraph previews, save/reopen/re-edit and search. Independent checks
matched all 80 saved runtime outputs for complete-page pixels, text/order and
source-font program sets. Text comparison retains the audit's existing
physical-line trailing-ASCII-space normalization. The owned desktop process was
stopped and its absence checked. These runtime cases use the authored two-piece
uniform and three-piece nested paragraphs; they do not establish automatic
grouping of arbitrary headings, columns or wider-leading source rows.

Earlier candidate acceptance remains associated with its original source;
it is not substituted for acceptance of the corrections. The completed corrected
executions bind source fingerprint
`20e9bf715461f005bd72cefdbc29b5187ee91e356d293968a81db43c1d756074`.

The source-proof limits remain 2,000,000 charged CFF work units per program,
16 MiB per program, 128 MiB total programs, 64 resources, 2 MiB of ToUnicode data
and a 4,096-byte edit limit. Unsupported encodings, conflicting or missing
mappings, unsafe transforms and unsupported shaping remain explicit refusals.
These fixture-level checks do not certify arbitrary producer fonts, complex
scripts, application-wide performance or Adobe/Foxit parity.

The [design](superpowers/specs/2026-11-12-source-cff-design.md) and
[implementation plan](superpowers/plans/2026-11-12-source-cff.md) define the
component's remaining acceptance requirements.
