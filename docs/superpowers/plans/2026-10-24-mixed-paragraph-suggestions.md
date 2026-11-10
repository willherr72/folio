# Mixed Paragraph Suggestions Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline, task by task.

**Goal:** Open a complete geometrically supported mixed-format paragraph from a clicked piece.

**Architecture:** Add a mixed mode to the existing geometric suggestion helper,
then validate the candidate through styled inspection. Saved memory wins; failed
fresh candidates fall back to clicked-piece inspection.

**Tech Stack:** Rust/PDFium/lopdf native engine, existing React paragraph dialog.

**Spec:** ../specs/2026-10-24-mixed-paragraph-suggestions.md

## Constraints

- Preserve uniform suggestion behavior and all publication/source-fidelity gates.
- Fresh groups at most 64 pieces; saved groups retain existing verified limits.
- No private words or PDF copies in artifacts; numeric-only private diagnostics.
- Existing dirty worktree and prior executable/checkpoint artifacts are preserved.
- User authorized autonomous implementation; no new approval gate or commits.

## Tasks

- [x] Add failing native tests for fresh mixed suggestions and boundary cases in
  `src-tauri/tests/styled_paragraph_suggestion.rs` and an authored integration file.
- [x] Add `paragraph_selection::suggest_styled` / shared mode. Mixed rows use
  maximum size for leading and common body-size evidence across the whole block.
  Retain uniform wrapper and all neighborhood/group limits.
- [x] In `text_paragraph.rs::suggest_styled_paragraph`, compute fresh geometry
  and extents, restore saved memory first, inspect fresh group, then fall back.
- [x] Run native suggestion/uniform/highlight suites and connected dialog tests.
  Probe quiz membership without exporting or logging text. Record regressions.
- [x] Request scoped independent review, resolve findings, document feature and
  limits, and preserve a fingerprinted checkpoint. Windows/build acceptance follows
  before claiming a new runnable milestone.

Corrected checkpoint: `artifacts/mixed-suggestions-corrected-20261024/verification.json`.
21 native passes,15 dialog passes, successful TypeScript/Vite/debug build, four
Windows rotations/eight independent saved-output comparisons. Review2 closes P2
local-gap issue. Private API probe asserts consistent seven-piece membership and
original-font insertion in the suggested area. No private export or parity claim.

## Review focus

Cumulative baseline/size drift; misleading forged saved hints; skipping unsupported
or whitespace pieces; columns/lists/headings; failure fallback and stale anchors;
whole-block common size evidence; source order and pixel/source immutability.
