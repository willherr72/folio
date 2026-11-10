# Source-owned native area selection

## Intent

Close the private combined-publication API's trust gap: a caller currently supplies text ranges separately from geometric marks. Add a WorkerRuntime operation that accepts only a source ID and page rectangles, derives all intersecting admitted source glyphs itself, and invokes combined publication. This advances the complete area-redaction design; images, reusable forms, clipping and end-user UI remain later requirements, not silently accepted omissions.

Standing autonomous authorization applies. Preserve 0.30 and existing text-only behavior. No new dependency, public command, version bump, commits, private-file use or package replacement.

## Design

Extend the existing source/native glyph binder with an explicit complete-area measurement mode. The existing `inspect_redaction_text_page` continues to skip spaces exactly as before. The new mode includes source Unicode U+0020 and native non-generated spaces, using native loose boxes for spaces and tight boxes for other glyphs. Bind every native measurement one-to-one to the structurally inspected source by Unicode/origin using the established bounded ambiguity checks. No ambiguous or missing source/native character is guessed. Unsupported or degenerate glyph measurements refuse. Generated layout separators do not authorize source removal.

The resulting selection records whether complete-area measurement was requested. Its area-range method refuses a text-only measurement. It validates finite, positive, page-contained top-left marks, uses inclusive boundary intersection, deduplicates overlapping marks and merges only contiguous selected source offsets. It does not bridge unselected spaces. Unlike text-only selection, a rectangle need not hit text: drawings-only marks must remain usable.

`WorkerRuntime::redact_content_areas(source, marks)` captures immutable source bytes and hash, measures each marked page in complete mode, converts the same marks to bottom-left points, and invokes `remove_planned_content` with internally derived ranges. It returns a private result containing the source identity/hash, output and selected glyph boxes for future preview disclosure. It neither registers nor adopts an output. Before returning, it verifies the captured source allocation/hash still match. Invalid marks or unsupported content return no partial result and do not alter document/session ownership.

Limits: 1..64 marks, pages below1000, at most4096 ranges and250,000 selected glyph boxes across the whole request; existing per-page/native/source/parser/output limits remain. Source access uses existing editing/copying/protection policy checks. Input points must convert to representably positive rectangles. Full source scanning per marked page is bounded but not claimed optimized; do not introduce a mutable cross-source cache.

## Alternatives

Trust frontend-selected ranges: rejected, since geometry completeness would remain unproven. Add a separate duplicate glyph matcher: rejected because it could diverge from established ambiguity safeguards. Reuse the binder with a private explicit complete-mode flag: selected, keeping current text-only semantics and tests intact.

## Verification

Failing native regressions precede implementation. Cover text+drawings through only marks, space-only marks, overlapping marks, white/invisible-by-color text, drawing-only and blank marks, source immutability and no registered candidate, invalid/closed/print-only sources, missing/ambiguous glyph binding and finite/contained geometry. Include reflected or rotated admitted text and generated separators where feasible. Run all redaction-related native tests serially, including existing text-only tests.

Generate an owned two-page original and saved output using the existing publication fixture programs, with fixed marks that select SECRET and the red path. Reuse the independent saved-output audit's exact source glyph preservation, removal/graph checks, full expected rasters and corruption controls. Add a saved whitespace-only output with exact source operands absent and retained glyph positions unchanged. Source/native binding and all request ranges are internal; no caller-supplied selection IDs are accepted by this new entry point.

One final scoped review and coordinator correction pass close the milestone. This does not establish arbitrary-font geometry, images, clipping, full host preview/adoption or general secure-redaction certification. Those remain necessary before end-user exposure.

## Evidence-driven whitespace correction

The first complete-mode implementation failed two required native regressions: consecutive spaces and a second edit after removing a word. The read-only `space-diagnosis/report.json` confirms PDFium discards consecutive U+0020 characters from extraction even when all literal source operands remain. Missing measurements still refuse; do not reduce the cardinality proof.

For pages containing spaces, build a bounded temporary one-page measurement document from the strict canonical source. Admit only the twelve unembedded standard Latin fonts already validated by the structural kernel, with their existing standard/WinAnsi rendering encodings. Add a sparse ToUnicode mapping from byte32 to U+E000. This changes extraction identity, not rendering encoding, widths, text operators, or matrices. Native loose boxes for the mapped characters bind back to the original source space offsets/origins. All non-space glyphs also bind against the original native document and must have exactly equal measured boxes in the probe. Every source glyph must still bind uniquely. Generated separators remain excluded.

The probe is local measurement data, never a registered document or published edit. Serialization has the same64MiB ceiling. Embedded/custom fonts on pages requiring this workaround refuse; extending them requires separate glyph/rendering proof. Original non-space-only and text-only measurement behavior remains unchanged. A diagnostic experiment established all missing spaces and equal full-page independent rasters for the owned fixtures; permanent native tests and actual output audits remain acceptance requirements.

Because this is a material measurement design change after the initial review, obtain one fresh scoped review of the correction. Small follow-up findings remain coordinator-owned. The initial report and failed native evidence remain intact.
