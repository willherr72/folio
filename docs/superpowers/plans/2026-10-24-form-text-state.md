# Form Text State Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Edit supported nested-form text that uses ordinary PDF spacing, scaling,
rise and leading instructions without changing other occurrences or source state.

**Architecture:** Keep the existing resource-chain isolation and stream-preserving
writer. Track effective text state in the private flatten-validation model and
each occurrence. Preserve original operators and gate encoding-dependent font
changes per occurrence, enforced natively and explained in the existing dialog.

**Tech Stack:** Rust, lopdf, PDFium, TypeScript/React, Vitest; generated PDF fixtures,
MuPDF/pypdf independent readers, owned Windows WebView2 acceptance.

**Spec:** `docs/superpowers/specs/2026-10-24-form-text-state-design.md`.

## Global constraints

- Existing dirty isolated worktree; inline implementation; no routine permission
  checkpoints, commits, source-document overwrite, or external release.
- Preserve actual no-op raster/glyph equality, original/flat equality, verified
  encoding, other-occurrence state and geometry, ancestor clipping and collisions.
- Scalars finite, magnitude <=1,000,000; Tz 0.01 <= abs(value) <=10,000; Tr integer0.
- Same-font edits retain source encoding. New Identity-H font installation is
  refused for effective nonzero Tw with a single-byte source font.
- Preserve original image-export executable and baseline fingerprint:
  `artifacts/page-image-export-20261024/reviewed-ownership/build-source-fingerprint.json`.

## Review focus

- BT does not reset text state; q/Q and nested forms do restore it (Task1 fixtures).
- State after the selected Tj can affect subsequent objects, not the selected
  occurrence's capability or recorded state (Task1/2 scope fixtures).
- Tw semantics follow source character-code width, not Unicode whitespace or a
  page-global flag; a sibling must not block substitution (Task2).
- Scale/rise can enlarge or move ink; all ancestor and neighbor checks must still
  refuse unsafe replacements (Task3).
- Save/reopen, source protection and source-version history must preserve state
  and isolate shared occurrences (Task3/native existing regressions/desktop).

## Task 1: Source state and occurrence preservation

Files: new `src-tauri/src/form_text_state.rs`, `src-tauri/src/form_text.rs`, new
`src-tauri/tests/form_text_state.rs`, fixture helper under `tests/support` if useful.

Interfaces: `TextState` is Copy/PartialEq with PDF defaults, finite scalar fields
`character_spacing`, `word_spacing`, `horizontal_scale`, `rise`, `leading`.
`TextState::apply(&mut self, operation: &Operation) -> EngineResult<bool>` returns
false for operators outside its domain. Tr only admits zero. `Occurrence` records
the state at Tj. `flatten()` receives inherited state and saves/restores it with
the existing transform stack; recursive calls receive copies.

- [x] Write failing native fixtures: default state operators list as supported;
  unchanged, shorter and longer same-font replacement preserve exact authored
  reference pixels and other copies. Run `cargo test --test form_text_state`.
- [x] Add the bounded state helper and per-occurrence tracking; preserve source
  operators. Compare corresponding occurrence states during publication.
- [x] Add state-scope and invalid-operand tests: page/wrapper/leaf inheritance,
  q/Q, BT persistence, post-Tj changes, sibling form restoration; malformed/huge
  numbers, invalid scale, non-fill modes. Keep rejection tests meaningful.
- [x] Run new tests plus existing `form_text`, `embedded_form_text`, `form_affine`.

## Task 2: Honest font substitution capability

Files: form_text.rs, types.rs, src/editor/types.ts, ExistingTextDialog.tsx and
corresponding native/dialog tests only as needed.

Interfaces: use existing `VerifiedFont::single_byte()` (standard fonts are single
byte). Helper `can_substitute(run: &RunSnapshot, occurrence: &Occurrence) -> bool`
is true when Tw is zero or the source font is verified two-byte. Enforce after
existing same-program/encoding reuse resolution: an actual new font installation
must pass the helper. Optional `fontSubstitutionReason` on FormTextRun/EditableTextRun
communicates the original-font-only limitation without disabling supported editing.

- [x] Add failing substitution tests: Tc/Tz/Ts retained; single-byte nonzero Tw
  capability false and forged/direct new-font request rejected; Identity-H Tw
  allowed; zero override and a sibling's Tw do not block the selected occurrence.
- [x] Implement native capability and enforcement; include the explanatory reason
  in the existing dialog if absent. Exact verified encoding reuse remains allowed.
- [x] Add a dialog regression showing the reason, editable replacement input and
  successful original-font submission, with no substitute picker offered.
- [x] Run form_font/form_affine/new native tests and affected frontend tests.

## Task 3: Compatibility and runnable acceptance

- [x] Capture generated standard/simple/CID outputs across selected state scopes,
  page rotations, affine transforms, CropBox and decimal UserUnit. Compare with
  independently authored references via MuPDF/pypdf; verify source/other pages,
  outside-region pixels, extraction, retained source state and repeated re-edit.
- [x] Retain clipping/neighbor rejection and protected/recovery regression gates;
  run relevant native suites, full frontend if changed, and TypeScript build.
- [x] Request independent scoped review, resolve actionable findings with regression
  evidence, preserving separate coverage limits.
- [x] Build fresh fingerprinted executable. Owned Windows edit/undo/redo/save/reopen
  and original-font-only feedback acceptance; inspect outputs independently.
- [x] Update existing-text docs, quality loop, ledger and verification record.
  Keep the broad goal active and state remaining font/script/corpus limits.

Self-review: state admission/scoping, font encoding, preservation, UI explanation,
tests and final output/lifecycle gates map to the three tasks. All choices retain
the approved architecture; builder check reported nothing important left.
