# Source-order Paragraph Implementation Plan

> Use superpowers:executing-plans inline. Preserve dirty worktree and evidence;
> one final review, with demonstrated corrections inline. No commits/publishing.

**Goal:** Preserve the selected paragraph's position in saved PDF text order.
**Architecture:** Bounded state-preserving source splice, internal emitted ranges,
then independent native text/font/glyph/pixel and saved-memory verification.
**Tech Stack:** Rust/lopdf/PDFium, existing Tauri Windows acceptance harness.
**Spec:** docs/superpowers/specs/2026-10-26-paragraph-order-design.md.

## Constraints and review focus

- Keep original bytes, neighbor glyphs/positions, fonts, comments and graphics.
- Preserve line matrix as well as current cursor, including q/Q and quote state.
- Pending path reconstruction must not repaint existing content or lose paths.
- Generated text/highlight ranges are internal evidence, not trusted PDF hints.
- Repeat edits must not accumulate state replay wrappers or lose other drafts.
- Retain existing page/content/operation/depth limits and independent gates.

## Task1: Source-order publication

Files: new src-tauri/src/styled_paragraph_splice.rs;
src-tauri/src/{text_paragraph,styled_paragraph_publish,paragraph_highlight_publish}.rs;
new src-tauri/tests/styled_paragraph_order.rs.

- [x] Reproduce before/paragraph/after extraction failure through native preview.
- [x] Implement checked prefix-state reconstruction and cursor restoration;
      unit-test transformed cursors, nested scopes, paths and malformed limits.
- [x] Insert canonical fragments at original span; return internal range/ordinal.
- [x] Use emitted range for highlight generation and final saved-memory checks;
      verify retained neighbors on both sides of the new paragraph.
- [x] Pass direct/relative/transformed/highlight/repeated-edit source-order tests.

## Task2: Integration and acceptance

- [x] Run affected layout/publication/memory/highlight/physical/occurrence suites.
- [x] One final scoped review; reproduce/fix findings inline, rerun affected gates.
- [x] Fresh Windows build, quiz UI and copy order, independent MuPDF unsorted
      extraction, baseline and pixels; preserve failed evidence and source hashes.
- [x] Update quality record, dispositions and ledger; keep overarching goal active.
