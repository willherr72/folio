# Paragraph Fit Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline.

**Goal:** Fit a current paragraph draft into available page space without changing typography.

**Architecture:** Factor shared native admission/measurement from layout, reuse
bounded area search and obstacle collection, expose a bounds-only engine command,
then add an asynchronous undoable dialog action.

**Tech Stack:** Rust/PDFium/lopdf, Tauri, TypeScript/React, Vitest/Playwright.

**Spec:** ../specs/2026-10-24-paragraph-fit.md

## Constraints

Current draft limits and native fidelity gates remain. No automatic font/size
substitution, source overwrite or preview bypass. Preserve intentional worktree
changes and old artifacts. User authorized autonomous implementation, no commits.

## Tasks

- [x] Native failing tests: overflowing current draft fits; no-room/obstacle and
  unchanged results; formatting/highlight/substitution/crop/rotation and invalid requests.
- [x] Extract measured plan in styled_paragraph_plan.rs; implement fit in
  styled_paragraph_area.rs with common obstacle collection and clear-area checks.
- [x] Wire engine worker, app command and adapter; test complete request transport.
- [x] Add dialog action and tests for request/history/preview preservation,
  no-op/failure, late responses, source changes, and temporary font leases.
- [x] Verify native/frontend/build, run Windows acceptance and independent saved
  output checks, obtain review, fix findings and record source/artifact fingerprints.

## Review focus

Request-validation drift during refactor; invalid/occupied initial rectangles;
raw/displayed rotation mapping; source-highlight exemptions; no-op history and
preview ownership; late requests/font leases; bounded search and failure semantics.

Completed checkpoint: artifacts/paragraph-fit-corrected-20261024/verification.json.
Broad native202/3 preceded the semantic legacy-normalization correction; final
27 targeted native passes include the actual editor request bridge. Frontend823,
corrected Windows build, four desktop rotations/eight independent saved outputs.
Review2 closes the reproduced legacy policy-switch P2; broader gaps are retained.
