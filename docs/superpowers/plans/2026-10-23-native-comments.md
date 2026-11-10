# Native Comments Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline, with one focused fresh review after implementation.

**Goal:** Read and edit standard fixed-icon sticky notes without replacing their PDF appearance.

**Architecture:** Track imported annotation indices in immutable engine sources. A bounded native codec reads remaining Text notes and patches Contents in a source copy. Review and a modal use optional adapter commands and commit a full-source replacement.

**Tech Stack:** Rust, lopdf, PDFium, Tauri, React, Vitest, Playwright.

**Spec:** docs/superpowers/specs/2026-10-23-native-comments-design.md

## Global constraints

Preserve all original PDFs, uncommitted work, and portable artifacts. No new dependencies. Native permissions are authoritative. Do not convert fixed-icon notes to overlays. Keep unsupported comments readable with a reason. Execute inline without approval or commits.

## Review focus

- Imported overlay comments must never appear twice or reappear after removal.
- Undo/recovery/reopen must not convert a native edit to an unowned overlay.
- Shared source pages and duplicated workspace pages update together.
- Stale asynchronous results must not change another tab or leak a source.
- Metadata, custom appearance, popup, permissions, and signatures survive or cause a clear refusal.

### Task 1: Native codec and engine commands

Files: create src-tauri/src/native_comments.rs and src-tauri/tests/native_comments.rs; modify engine.rs, types.rs, lib.rs, app.rs.

Interfaces: NativeComment {pageIndex, annotationIndex, text, author?, subject?, bounds, reason?}; EditNativeCommentRequest {pageIndex, annotationIndex, expectedText, text}; PdfEngine::list_native_comments(sourceId), edit_native_comment(sourceId,request). Track imported original indices in OpenDocument; list excludes them.

- [x] Write and run failing integration tests for F28 versus imported F4 notes, crop/rotation bounds, exact metadata and pixel preservation, Unicode edits, stale expected text, direct dictionaries, locked/rich/hidden refusals, permissions, export/reopen and immutable originals.
- [x] Implement bounded parsing and Contents-only mutation, signature refusal, source replacement with inherited policy and imported-overlay stripping.
- [x] Run native comment tests plus annotation/form/recovery regressions.

### Task 2: Review UI and ownership

Files: create native-comments.ts hook/model, NativeCommentDialog.tsx, relevant tests; modify adapter.ts, types.ts, ReviewList.tsx and App.tsx.

- [x] Write failing model and UI tests for listing/navigation, edit/undo, duplicates, stale results, source cleanup, permission/reason and error states.
- [x] Load comments by immutable source and map to current page order; merge with overlay Review entries. Apply native edit as one full-source undo step, preserving overlays and bookmarks.
- [x] Run focused frontend tests and production build.

### Task 3: Acceptance and review

- [x] Verify actual browser and Windows application workflows, save/reopen and independent PDF inspection; run appropriate complete suites without overlapping Cargo processes.
- [x] Request one focused fresh review, fix important findings with regression tests, rerun affected acceptance checks.
- [x] Record evidence and remaining gaps in ledger and product documentation. Keep overall parity goal active.
