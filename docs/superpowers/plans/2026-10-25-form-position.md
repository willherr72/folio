# Existing field positioning implementation plan

> Use superpowers:executing-plans inline. Standing authorization removes routine
> approval questions. One final scoped reviewer; preserve dirty work and evidence.

**Goal:** Move and resize existing PDF widgets without losing their field identity.
**Architecture:** Rect-only native mutation on a complete immutable source, published
through PageContentReplacement; dedicated dialog reuses existing preview ownership.
**Tech stack:** Rust/lopdf/PDFium, React/TypeScript, Windows WebView2/CDP.
**Spec:** docs/superpowers/specs/2026-10-25-form-position-design.md

## Global constraints

Source displayed physical points; minimum 4 pt dimensions; expected-bound tolerance
0.01 pt. Retain all appearance streams and values. Resize scales saved appearances.
Bound graph/selected-field inspection; original inputs untouched. No commits/releases.
One Cargo/build at a time. Existing isolated worktree, no AGENTS found.

## Review focus

- Shared widget aliases and sibling field identities: one intended placement changes.
- Intrinsic/workspace rotation, crop origin and physical units: hit area matches ink.
- Appearance/value preservation and later filling after scaling or moving.
- Late async results, stale drafts and foreign-source candidate ownership.
- Permission/locked/signed/malformed sources, graph copy budgets and rollback.

## 1. Native immutable geometry transaction

Files: new form_position.rs/form_position_engine.rs, engine.rs/lib.rs/app.rs;
new tests/form_positioning.rs. Input PositionFormFieldRequest; output
PageContentReplacement with original source occurrence mapping and a fresh source.

- [x] Add a native test for moving a filled field, saved geometry/value and source
  immutability. Run cargo test --test form_positioning; expect absent API failure.
- [x] Implement validation, Rect-only mutation and selected-widget verification;
  reuse duplicate isolation, permissions, bookmarks, source registration.
- [x] Expand native tests for rotations/units, resize AP preservation, shared fields,
  duplicate positions, invalid/stale/locked/alias cases and protected permissions.
  Run focused and related regression; expect all supported cases pass and refusals
  leave source bytes and source registrations unchanged.

## 2. Desktop positioning controls

Files: editor/form-positioning.ts, adapter.ts, FormPositionDialog.tsx,
FormFillEditor.tsx, App.tsx; frontend model/dialog/app tests.
Consumes the native request and PageContentReplacement contract from Task 1.

- [x] Write failing selection/position/preview/ownership tests; observe failure.
- [x] Implement entry from selected field, point controls and draggable keyboard
  handles, clear scaling explanation, crop validation and candidate disposal.
- [x] Wire one-step commit and close filling after Apply; test Undo/Redo, dirty draft
  gating and stale/borrowed results. Run focused tests, full frontend and build.

## 3. Review and accepted checkpoint

- [x] Freeze cycle baseline/delta and dispatch one reviewer with spec and focus.
- [x] Reproduce/fix demonstrated important findings red-to-green, no rereview.
- [x] Run Windows move/resize, keyboard/narrow preview, cancellation, Undo/Redo,
  fill and save/reopen. Independently verify untouched pixels and shared values.
- [x] Bind source/binary/PDFium hashes, stop owned app, update quality ledger/README.
  Keep broad goal active; explicitly report coverage limits.
