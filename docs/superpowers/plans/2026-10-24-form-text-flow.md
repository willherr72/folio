# Reusable Text Font and Line Flow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline. Steps use checkboxes.

**Goal:** Edit ordinary reusable multiline PDF text, preserving active fonts, independent line positions and untouched occurrences.

**Architecture:** Add a bounded text-flow admission helper, carry reference-counted resolved font selections across graphics scopes, and bracket a substituted show with local select/restore Tf operations. Keep source operators and existing native publication proofs.

**Tech Stack:** Rust/lopdf/PDFium; ReportLab fixtures; MuPDF/pypdf; owned Windows WebView2.

**Spec:** `docs/superpowers/specs/2026-10-24-form-text-flow-design.md`.

## Global constraints

- Existing isolated dirty worktree; preserve accepted source fingerprint `artifacts/form-positioned-20261024/build-source-fingerprint.json`, executable and user PDFs. No commits, deletion, publishing or routine approval prompts.
- Keep existing 100,000 expanded operations, 5,000 occurrences, depth12, font/graph CopyBudget, numeric finite abs<=1,000,000, font size(0,1000], form Tj/TJ byte8192/array16,384 bounds and source proof rules.
- No UI/dependencies. General adjacent shows dependent on prior glyph advance remain refused; independently positioned shows and quotes are supported. Do not flatten published forms.
- Keep actual source/private-flat/no-op, glyph/encoding/other-run, ancestor/collision/outside-raster guards and Tw substitution policy.

## Review focus

- Ancestor font selection with a child shadowing the same resource name must restore the actual selected font after substitution (Task2).
- q/Q and sibling-form return must restore font and leading; BT/ET must retain them (Task1/2).
- Double quotes update Tw/Tc before font-substitution eligibility and retain that state after the show (Task1/2).
- Numeric-only TJ must neither consume paths nor turn a dependent next show into an independent one (Task1/2).
- Shortening/lengthening one line must leave later line origins/fonts and shared copies exactly unchanged, including TD/T* and transformed placements (Task2/3).

## Task 1: Text-object and line-flow admission

Files: new `src-tauri/src/form_text_flow.rs`, `src-tauri/src/form_text.rs`, new `src-tauri/tests/form_text_flow.rs`.

Interfaces:
- `TextFlow::position(&mut self, operation:&Operation, state:&mut TextState)->EngineResult<bool>` handles balanced BT/ET and valid Tm/Td/TD/T*; exposes `in_text()` and `finish()` for graphics checks/end validation.
- `TextFlow::show(&mut self, operation:&Operation, state:&mut TextState)->EngineResult<Option<Vec<u8>>>` validates Tj/TJ/quote syntax, applies quote state/line reset, returns None for numeric-only arrays or bytes for a glyph show, requiring independence from the prior glyph advance.
- `SelectedFont {resource:Object,size:Object}` held as `Arc<SelectedFont>` stores the resolved font and exact size operand. Occurrence adds that pointer. Flatten receives optional inherited selection and saves/restores it alongside transform/TextState; Tf updates it and flat alias at any valid text-state position.
- Form `glyph_show` includes single/double quotes. Replacement quote updates only its final string operand; numeric TJ uses the existing shared helper.

- [x] Write RED native fixture using empty font setup, later BT without Tf, multiple Tj/TJ/quote lines and Tm/Td/TD/T*. Assert four stable occurrences and independent first/later original-font edits; run `cargo test --test form_text_flow`.
- [x] Implement helper and integrate font-state/admission/path mapping. Add unit tests for balanced empty objects, TD leading, quote spacing, reset dependency, numeric cursors and malformed counts/outside-text commands. Run focused unit/new native tests.
- [x] Exercise original ReportLab drawString/textLine/explicit-line probes with native listing and edit/save checks. Keep source/reference streams for independent verification.

## Task 2: Per-show font isolation and preservation regressions

Files: `form_text.rs`, `form_text_flow.rs`, `tests/form_text_flow.rs`, reuse existing fixture helpers without broad refactoring.

Interfaces:
- `clone_chain` receives selected occurrence's `&SelectedFont` alongside existing replacement and optional installed font; resolves target path as before.
- On installed font, fresh alias for new resource and fresh restore alias for effective original resource in the selected leaf; insert Tf/show/Tf in place of the single target op. CopyBudget accounts actual direct graph copies. Keep earlier selections and existing font entries. Same-font branch substitutes only the show.

- [x] Add RED font-choice tests on first and later line with inherited fonts, q/Q restoration and shadowed names. Assert later lines/shared copies retain fonts/positions and native saved pixels; source immutable. Implement select/restore.
- [x] Add standard/simple/CID, quote Tw/Tc policy, TD leading persistence, font-size changes, empty setup scopes, local alias collision and numeric cursor path cases; reject malformed syntax, unknown fonts, dependent adjacent shows and split codes.
- [x] Add affine/rotation/UserUnit, clip/collision, protected source and recovery acceptance with authored references. Run new tests and `cargo test --lib --test form_text --test form_text_state --test form_positioned_text --test form_font --test form_affine --test embedded_form_text` with one Cargo worker and milestone logs.

## Task 3: Independent and runnable acceptance

Files: new reader and Windows scripts, milestone artifacts, existing-text documentation/quality loop.

- [x] Save originals/authored references/edited PDFs from native cases. Independent readers compare text, geometry and raster, selected operator/state/slot preservation and untouched lines/pages; keep meaningful initial failures and corrections.
- [x] Request one fresh scoped implementation review against accepted positioned baseline, handle demonstrated findings with regression evidence. Run TypeScript/debug build and record unchanged frontend hashes (prior861 tests only if still unchanged).
- [x] Use owned Windows/native-dialog workflow for real ReportLab and embedded/quote/transformed cases: actual targets, longer edit, explicit substitution, undo/redo, save/reopen and re-edit; inspect screenshots and independently verify saved copies. Close owned processes.
- [x] Record executable/source fingerprints, unique test counts, review, immutable inputs and gaps. Mark plan complete only after gates; keep broader goal active.

Self-review: task1 defines exact flow/font interfaces used by task2; all five review-focus cases have task2 regressions. Task3 proves actual output and runnable behavior, not just listing admission.
