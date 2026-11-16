# Source-font editing status

Current development source can edit supported text containing source ligatures
such as fi and ffi in eligible embedded CID TrueType fonts. A PDF can draw several
letters as one physical glyph; Folio retains that identity instead of treating
the glyph as unrelated letters with guessed widths. Supported individual runs,
paragraphs and reusable form content use the verified source mapping. Explicit
font substitution remains a separate choice when the existing subset cannot
represent the replacement.

A newer source component extends this model to eligible CFF source ligatures,
including verified simple-font codes. Its native, frontend and normal optimized
GUI/CLI checks passed, along with actual CLI/MCP/Windows edits and independent
saved-output audits. Its final scoped review found an ink-proof defect; the
correction passed fresh native, parser, frontend, build and compiled-workflow
qualification. Conservative contour-area checks can exclude unusual outlines. The
[CFF status](source-cff-status.md) records the precise scope and limits.
It is separate from the completed 0.54 portable.

Some eligible CID TrueType fonts omit an intrinsic Unicode cmap. Folio can reuse
them when the PDF supplies verified Unicode, character-code and glyph mappings,
and the embedded outlines and widths agree. Missing source meaning, ambiguous
aliases, unsupported mappings and unsafe layouts still prevent an edit. A font
name alone never proves eligibility. Source-font names and keep/substitute
guidance are clearer in the current dialogs.

The corrected source passed native original/substitute root and shared-form
edits, saved re-editing, exact no-ops and paragraph checks. Independent-reader
audits passed 52 saved-file pairs and rejected 20 deliberate corruptions without
enlarging masks around the original text. Fresh compiled CLI and owned Windows
desktop workflows passed, including Undo/Redo and save/reopen. Source and font
program/glyph identities and surrounding content were checked separately.

This is bounded source-ligature support. It does not enable general complex-script
shaping, bidi or combining-sequence editing, arbitrary unproved multi-scalar font
maps, arbitrary reassignment of real kerning gaps, every producer or every reader,
zoom/DPI and printer. When a safe cursor correction cannot be proved, the edit
refuses and paragraph editing remains a separate option.

These completed component results are included in the reviewed separate 0.52
portable. Its extracted-package acceptance and standalone seal are complete;
see the [current build status](development-build-status.md) for exact evidence.
Selected 0.50 and the reviewed separate 0.51 candidate remain preserved. Physical
system clipboard/IME and broad memory, performance, accessibility and commercial
parity acceptance remain open. No private PDF or clipboard data was used in these
new public-fixture audits.
