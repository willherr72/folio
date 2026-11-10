# Canvas Form Filling Implementation Plan

> Use superpowers:executing-plans inline, TDD, and one final scoped reviewer.

**Goal:** Select form fields on the main canvas and fill them with persistent,
accessible controls while retaining navigation and source-safe transactions.
**Architecture:** Share the existing form editor between a Properties host and
optional modal overview. Add source-bound canvas targets, then distinguish read
navigation from blocked document mutation while a form session owns a draft.
**Tech stack:** React/TypeScript, existing Rust/PDFium form APIs and Windows harness.
**Spec:** docs/superpowers/specs/2026-10-24-canvas-form-filling-design.md.

## Constraints and review focus

Preserve dirty workspace and originals; no commits/release. New mode must not
inherit annotate permission. No invisible cropped keyboard targets, duplicate
draft owner, implicit discard, stale load installation or native source leak.
Explicitly test same-field reselection, navigating while dirty, late native
completion, protected fill-only access and NoRotate cropped targets.

## Implementation and checks

- [x] Add failing app tests in tests/canvas-form-filling.test.tsx for entering
  without a modal, selecting on-page fields and navigation with a retained draft.
- [x] Extract reusable FormFillEditor from FormFillDialog, keeping existing
  controls and overview tests; add controlled external selection/dirty callbacks.
- [x] Add crop-aware reusable field targets and CanvasFormLayer with stale-load
  protection; test geometry, keyboard focus, denied fields and source changes.
- [x] Integrate through PageView, DocumentViewport and App. Keep form owner
  independent from visible-page selection; retain shared-source apply and orphan
  disposal. Enable only read navigation/zoom during the session; guard mutations.
- [x] Verify existing and new form tests, permissions, viewport, full frontend
  suite and build. Reuse native form regression only if native behavior changes.
- [x] Freeze scope, request review, fix reproduced findings, stage a fresh build,
  run Windows form acceptance and independent saved-file checks.
- [x] Record checkpoint hashes, completed workflows and remaining gaps in the
  quality ledger. Keep the overall Adobe/Foxit-quality goal active.

Evidence root: artifacts/canvas-form-filling-20261024. Retain the accepted subset
paragraph checkpoint and all existing user state.

## Completed scoped checkpoint

Final evidence: artifacts/canvas-form-filling-20261024/checkpoint-final/verification.json.
956 frontend passes; final 10 workflow tests and fresh build/typecheck pass. Two
reviewed product findings reproduced and corrected; final Windows run covers both
repros, seven native field kinds, save/reopen, shared fields and undo/redo. MuPDF
and pypdf verify saved values and outside-annotation pixels. Broader permission,
password, crop/rotation corpus and form-authoring gaps remain in the evidence.
