# Source-owned text mark implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans inline; one final scoped review.

**Goal:** Bind native glyph bounds to exact source operands and remove text selected by an owned on-page mark.
**Architecture:** Private engine child module inspects immutable source snapshots, maps actual Unicode/origin to source offsets, owns marks and revalidates source identity before invoking existing partial removal.
**Tech stack:** Existing Rust/lopdf/PDFium; prior independent Python saved-output auditor.
**Spec:** docs/superpowers/specs/2026-10-29-redaction-selection-design.md

## Constraints and review focus
No public command/UI/package/commit. Existing worktree and user autonomy. One offline locked Cargo with1job. Exact bounds/limits from spec:64MiB input/load decompression;250000source and500000native chars/page;0.004pt spatial cells,32candidates/cell;0.002pt origin comparison;4096ranges; existing full-document kernel budgets unchanged. Source boxes have no fallback and no caller authority. Ambiguity must refuse even after another match consumes a candidate. DefaultEncoding punctuation identity must use actual font decoding. Interior spaces may merge selected ranges but an unselected nonspace may never be bridged. Recheck current edit/copy rights and source hash before output; protected policy remains explicit refusal.

## Task1: native/source mapping and owned marks
Files: new src-tauri/src/redaction_selection.rs and redaction_selection_tests.rs; module wiring engine.rs; extend existing narrow font metrics helper in text_edit.rs and Glyph scalar in redaction_ranges.rs. Do not format unrelated inherited code.
Interfaces: WorkerRuntime::inspect_redaction_text_page(&self,&str,usize)->EngineResult<PageTextSelection>; PageTextSelection::glyphs()->&[MeasuredGlyph]; PageTextSelection::mark(TextMarkRect)->EngineResult<TextRedactionPlan>; WorkerRuntime::redact_text_mark(&self,&TextRedactionPlan)->EngineResult<RedactedTextRangesPdf>. Opaque owned inspection/plan, no deserialization or public construction. Native matching accepts reordered glyphs only if unique Unicode/origin binding; finite positive tight boxes convert to displayed top-left points.
- [x] Add stubs and native regression tests, run redaction_selection_ and observe missing-behavior RED.
- [x] Implement actual Unicode metrics and unique bounded native mapping, rectangles/interior-space merge, source/rights revalidation and immutable output; focused GREEN.
- [x] Add bucket/range/ambiguity boundary tests and any demonstrated regression before fixes. Preserve input/document-count invariants.

## Task2: saved-output and mapping acceptance
Add optional test exporter FOLIO_REDACTION_SELECTION_FIXTURES, inspect240 prior sources and record native/source mappings. Export48 mark removals for ordinary plain and TJ-middle across12faces/2encodings, requiring mark ranges exactly equal targetsecret. Reuse current independent auditor with expected48case mode in a new artifact copy; no old artifact changes.
- [x] Run all redaction tests and export native mapping/output evidence; diagnose failures without widening masks or inventing font exceptions.
- [x] Independently compare48saved outputs using both readers/pypdf, with prior source-only masks/graph checks. Check all240mapping receipts and retained origins/source identities.

## Task3: review and record
- [x] Freeze exact delta and completed evidence; one final implementation review, fixes locally RED/GREEN, no re-review.
- [x] Library gate and source hashes; checkpoint, quality ledger, next worker/UI integration tasks; overarching goal remains active.
