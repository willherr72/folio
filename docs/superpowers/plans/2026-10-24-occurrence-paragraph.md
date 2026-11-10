# Complete-source Paragraph Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline. Standing user authorization applies; no routine approval, commit, cleanup or release.

**Goal:** Native paragraph inspection and immutable full-source previews preserve shared fields and duplicate identity.

**Architecture:** Private checked projection reuses existing paragraph algorithms;
shared occurrence publisher grafts the proven output and validates it again.

**Tech Stack:** Rust/PDFium/lopdf; native integration tests and independent MuPDF/pypdf.

**Spec:** `docs/superpowers/specs/2026-10-24-occurrence-paragraph-design.md`

## Constraints and review focus

All spec contracts apply. Explicitly inspect projection policy/annotation ownership,
physical roundtrip, candidate/final cleanup, unrelated paragraph replay and source
resource binding. Legacy APIs must still refuse shared-field isolated publication.

## Tasks

- [x] Add failing uniform/styled shared-field occurrence regressions in
  `src-tauri/tests/page_occurrence_replace.rs`, exercising inspection, suggestion,
  preview, first/later/unique edits and replay.
- [x] Add `occurrence_paragraph.rs` with private projection lifecycle and public
  occurrence inspect/suggest/layout/fit/preview wrappers. Wire typed requests in
  `engine.rs`; extend physical conversion helpers with explicit occurrence mode.
  Extract common publication in `occurrence_text_edit.rs`, keeping preflight before
  candidate generation and all final proof/cleanup guarantees. Run focused tests.
- [x] Expand tests for shared fill/save/reopen, physical frames, protections,
  substituted fonts, highlighted styled content and failure/abandoned reply cleanup.
  Capture synthetic corpus and independently check text/pixels/interactions.
- [x] Run affected native regression suites, request one scoped review, resolve
  demonstrated findings, and persist source hashes, evidence and remaining limits.

Public API names: `inspect_occurrence_text_paragraph`,
`suggest_occurrence_text_paragraph`, `inspect_occurrence_styled_paragraph`,
`suggest_occurrence_styled_paragraph`, `layout_occurrence_styled_paragraph`,
`fit_occurrence_styled_paragraph` mirror existing read-only signatures.
`preview_text_paragraph_occurrence` replaces page_index with occurrences and selected
occurrence and takes ParagraphFormatting; styled preview mirrors it with its request.
Both return PageContentReplacement in physical units.

Completed native evidence: 328 regression passes / 4 opt-in checks ignored across
19 suites; one subsequent unrelated-paragraph test passes all four uniform/styled
combinations. Independent readers verify 30 cases / 88 pages and 16 filled pages.
Scoped review found no demonstrated defects. No desktop API routing/build was
included; that integration remains the next milestone. Accepted hashes/evidence:
`artifacts/occurrence-paragraph-20261024/checkpoint-accepted/verification.json`.
