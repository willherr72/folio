# Compound text-field answer preservation plan

> Execute inline with superpowers:executing-plans in the existing isolated worktree; one final fresh scoped review. Standing autonomous authorization applies.

**Goal:** Fill ordinary compound text-field appearances without losing their frame or clipping.
**Architecture:** Validate original appearance, prove static frame paint, canonicalize a text-only marked slot, reuse existing bounded transplant/publication.
**Tech stack:** Rust/lopdf/PDFium, Windows/Tauri acceptance, Python independent PDF/image readers.
**Spec:** docs/superpowers/specs/2026-11-01-compound-form-answer-design.md

## Global constraints
Preserve original/private PDFs, existing portable packages and dirty work. Keep malformed/ambiguous inputs guarded; respect existing byte/operation/graph budgets. No dependencies, commits or physical print jobs. No broad parity claim.

## Review focus
Frame accidentally treated as answer; answer clip widened; retained graphics/text state changed; nested marker/resource growth after clear/refill; newly admitted ambiguous paint or memory amplification.

### Task1: Bounded compound answer isolation
Files: src-tauri/src/form_answer_appearance.rs, new src-tauri/src/form_answer_compound.rs, src-tauri/src/form_answer_transplant.rs, src-tauri/src/form_answer_appearance_tests.rs.
- [x] Add exact PyMuPDF-program and frame/text-state/clip/ambiguity regressions; observe failure before implementation.
- [x] Canonicalize proved frame+text appearances into a text-only marked slot; charge copies before cloning. Reuse FramePaint with marker tokens removed only for geometric classification.
- [x] Preserve no-glyph state effects and stable marker ownership; keep distinct clips and unsupported paint refused. Run focused library tests serial/offline.

### Task2: Native publication and original-document acceptance
Files: new src-tauri/tests/form_compound_answer.rs and authored tests/fixtures/form-compound-pymupdf.pdf; artifact acceptance helpers under artifacts/compound-form-answer-20261101.
- [x] Reproduce original whole-document refusal, then pass fill/clear/refill/save/reopen and source-byte preservation.
- [x] Verify exact unrelated-page/region pixels and original frame operations/defaults/resources; repeated edits remain bounded. Add variant and negative tests for state and ownership.
- [x] Run affected native suites and build a bound candidate serially; no source edits while gates/build run.
- [x] Use actual original fixture in desktop Apply/Undo/Redo/save/reopen and virtual printing, then independent saved-output audits. Original0.28 remains intact.

### Task3: Review and checkpoint
- [x] Freeze scoped code/evidence, obtain one final fresh review, correct consequential findings with regressions.
- [x] Verify exact terminal gates and fingerprint, update roadmap/continuation, keep overarching goal active.
