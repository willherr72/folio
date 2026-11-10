# Fresh Paragraph Baseline Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline. Preserve the dirty worktree and all prior evidence; no commits or publishing.

**Goal:** Keep the original first baseline when editing a freshly selected paragraph.
**Architecture:** Reuse validated wrapping/extent measurement for first-line ascent. Normalize each automatic-fit candidate before containment/layout checks. Saved hints and explicitly positioned requests remain unchanged.
**Tech Stack:** Rust, PDFium, lopdf, React/Tauri Windows acceptance.
**Spec:** docs/superpowers/specs/2026-10-26-paragraph-baseline-design.md.

## Constraints and review focus

- Original bytes, neighboring text/artwork, rotation/crop and physical units must remain intact.
- First-line font membership can change as a mixed paragraph widens; test the resulting baseline.
- Existing remembered areas, highlight padding and user-requested positions must not be silently rebased.
- Unfittable and page-edge cases retain safe bounded behavior; no unclamped allocations or unbounded fitting.
- No broad renderer/parity claims from focused tests; one final scoped review after implementation.

## Task1: Baseline-preserving fresh suggestions

Files: src-tauri/src/styled_paragraph_area.rs, styled_paragraph_layout.rs;
src-tauri/tests/styled_paragraph_initial_area.rs and focused additional regression.

- [x] Add public inspection/layout/publication tests demonstrating first-baseline drift for uniform/mixed font fixtures, rotations and UserUnit.
- [x] Run the new tests RED and retain the failure.
- [x] Extract shared validated wrapping/first-line ascent only as needed; anchor fresh fitting candidates to the source baseline before page/obstacle/layout admission.
- [x] Pass focused regressions, existing initial-area/fit/layout/memory/highlight/physical paragraph suites. Check unchanged input bytes and saved re-edit.

## Task2: Desktop fidelity and disposition

- [x] Build a fresh Windows checkpoint with source/renderer fingerprints.
- [x] Repeat quiz word insertion, preview/Apply, undo/redo, save/reopen and re-edit; measure first baseline and independent surrounding pixels against original.
- [x] Inspect extraction-order changes separately and record remaining work.
- [x] One scoped final review; fix demonstrated findings inline and rerun affected gates.
- [x] Update quality checklist and ledger; keep the overarching goal active.
