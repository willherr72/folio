# Text Display Size Implementation Plan

> Execute inline with superpowers:executing-plans, TDD, scoped code review.

Goal: show physical text size in editing dialogs without changing PDF operands.
Architecture: additive optional native measurement, existing unit conversion,
and compatible UI fallback. Rust/PDFium/lopdf, React/TypeScript.
Spec: docs/superpowers/specs/2026-10-25-text-display-size-design.md.

- [x] Add native display-size integration fixture and dialog label regressions;
  run failing tests without relying on private PDF content.
- [x] Add display_font_size to the three DTOs in types.rs; measure complete text
  matrix in RunSnapshot; populate text_edit.rs/form_text.rs/text_group.rs and
  scale the optional measurement once in physical_units.rs. Add optional TS
  fields and compatible label use in ExistingTextDialog/TextGroupDialog.
- [x] Run related native and frontend suites; independent scoped review, correct
  any reproduced findings, then build a source-fingerprinted executable.
- [x] Verify synthetic and original-quiz desktop size labels, editing semantics
  and private-source preservation; stop owned apps, record checkpoint and ledger.

Review focus: nested matrix composition, quarter turns/reflection, UserUnit
exactly once, raw field compatibility, group normalization and nonfinite values.
Evidence: artifacts/text-display-size-20261025. Prior checkpoints preserved.

Completed: 237 related native passes / 3 ignored; 964 frontend passes in 142
files; typecheck/frontend/native debug build passed. Scoped review found no
concrete defects. Windows direct/group/form edits, Undo/Redo, save/reopen and
independent MuPDF/pypdf physical-size/origin/text/surrounding-pixel checks passed.
Private quiz now shows 12 pt and insertion preview succeeds (not applied/exported).
Owned PID 28744 stopped and absence verified. See checkpoint/verification.json.
Review coverage gaps remain recorded; no release or parity claim.
