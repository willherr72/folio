# Word character-cell highlights implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline.

**Goal:** Reflow the original Word italic highlight with preserved source semantics.
**Architecture:** Add a separately proved cell-based background model; preserve the
legacy enclosing-ink route and its exact replay. Native saved geometry is authority.
**Tech stack:** Rust, existing PDFium/lopdf, current desktop/CLI paragraph API.
**Spec:** `docs/superpowers/specs/2026-10-29-word-highlight-cells.md`.

## Global constraints

Preserve original PDFs and inherited worktree. Locked/offline Cargo, one gate,
jobs1. No renderer/dependency/version/package changes or commits. Preserve generic
graphics/clip safeguards and resource budgets. Metadata v8 cannot weaken legacy
validation. Record incomplete acceptance honestly.

## Review focus

Adjacent source owners and foreground marks must not be adopted. Partial-word
rectangles cannot enter the whole-run route. Negative offsets cannot shrink text
reservations. Multiple formatting sizes must reserve every final paint box. Saved
metadata must bind exact current resources, geometry, operators and ownership.

## Task 1: Source proof and geometry

- [x] Add a failing native italic fixture to `paragraph_highlights_tests.rs`.
- [x] Add `paragraph_highlight_cells.rs` for bounded source-frame inference and
  reusable per-glyph paint geometry, with explicit negative tests.
- [x] Integrate optional cell backgrounds into discovery, layout reservations and
  native paint emission; preserve legacy paths. Files: `text_paragraph.rs`,
  `paragraph_highlights.rs`, `paragraph_highlight_geometry.rs`,
  `paragraph_highlight_publish.rs` and their existing tests.

## Task 2: Persistence and runtime acceptance

- [x] Gate cell backgrounds behind v8 in `styled_paragraph_memory.rs`; add forged,
  legacy-version and repeat-replay regression coverage.
- [x] Run focused and affected native suites with stable input fingerprints.
- [x] Build a fresh CLI and exercise the original Word source, including an
  explicit format change, save/reopen and repeated editing.
- [x] Audit independent readers, all other glyphs/pixels, highlights, links and
  nonempty tags; get scoped review and close findings.
- [x] Record desktop packaging/resource limits and the next goal milestone.
