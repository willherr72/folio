# Existing Form Frame Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Execute inline under standing autonomous authorization; one scoped final review. Preserve evidence; no commits or publication.

**Goal:** Edit existing widget borders and backgrounds while preserving answers, artwork and shared-field semantics.

**Architecture:** Bound and classify source appearance paint, replace only verified frame operations, and publish a private complete-source candidate. Reuse the occurrence transaction and existing form-dialog ownership patterns.

**Tech Stack:** Rust/lopdf/pinned PDFium, React/TypeScript/Tauri, Windows WebView harness, independent pypdf/MuPDF audits.

**Spec:** `docs/superpowers/specs/2026-10-26-form-frame-design.md`

Current status: all five tasks complete. The final verified record is

[checkpoint.json](../../../artifacts/form-frame-20261026/checkpoint.json).
Earlier checkpoint notes below are historical. No release or parity claim.

## Global constraints

- Preserve user PDFs, dirty worktree, old apps/executables and failed logs. No commits, pushes or releases.
- Source/output 512 MiB; form graph 4000 nodes/placements, 100000 values, depth 64; decoded appearance 4 MiB/262144 instructions; aggregate retained/generated work 64 MiB.
- Width is finite physical 0–32 pt, additionally limited by verified frame dimensions. Zero means no border.
- Atomic all-state, selected-widget/selected-occurrence editing; shared answers remain shared. No whole-appearance regeneration fallback.
- One Cargo/build at a time; use the existing shared CARGO_TARGET_DIR and CARGO_BUILD_JOBS=1. Log terminal exit and source fingerprint separately from review claims.

## Review focus

1. q/Q do not save paths; clip-bearing paints and state effects can alter later artwork. Task 1 state/path tests.
2. Labeled radios have small circles in wide widgets and distinct On/Off/Down programs. Task 2/3 all-state geometry and preservation tests.
3. Appearance aliases, inherited resources and repeated page occurrences can spread changes. Task 3 public isolation/refusal tests.
4. A larger border preserves today's text but changes later native padding. Task 3 fill, appearance and resize transition tests.
5. Transparent/missing paints, gray/CMYK colors and pressed-state shading can be confused with defaults. Task 1/2 palette and absence tests; Task 5 independent audits.

## Task 1: Bounded source frame geometry

**Files:** Create `src-tauri/src/form_frame_paint.rs`, `src-tauri/src/form_frame_paint_tests.rs`; register private module in `src-tauri/src/lib.rs`; expose a bounded generic lexical-preflight wrapper from `src-tauri/src/form_answer_appearance.rs`.

**Interfaces:** `FramePaint::inspect(bytes: &[u8], bounds: [f64; 4], radio: bool) -> EngineResult<FramePaint>` retains decoded operations and proved paint indices. `FrameShape` is Rectangle `[x,y,width,height]` or Circle `[cx,cy,outer_radius]`. `PaintColor` retains device components. Proof records border width/style/color and optional background, with state needed for safe replacement. No publication or UI exposure in this task.

- [x] Add tests for native rectangle ring/stroke, radio canonical circle and labeled-radio clip, preserving text operations. Add duplicate candidate, inner-art rectangle, transformed frame, unbalanced q/Q, q/Q-path lifetime, pending clip, nonfinite geometry, dashed state and gray/CMYK palette cases.
- [x] Run `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib form_frame_paint` and record failures for missing behavior.
- [x] Implement bounded lexical admission and finite graphics/path state classification. Admit only unique verified candidates; preserve all operations without interpreting text as frame paint.
- [x] Expand bounds tests for 4 MiB decoded input, 262144 operators, deep stacks and oversized operand arrays; exact BBox clipping is allowed, arbitrary frame clipping refuses. Run focused tests and existing `form_answer` tests; use task-done with the focused command after expected results are confirmed.

## Task 2: Frame metadata and paint rewriting

Progress checkpoint: metadata/request validation and common solid/dashed/underline
paint rewriting are implemented; 31 frame tests and 448 library tests passed
(three library tests ignored). Task 2 remains incomplete. See
`artifacts/form-frame-20261026/foundation-checkpoint.json` for exact hashes and
remaining admission/publication work. Do not restart task 1 or expose the API yet.

Later checkpoint: `artifacts/form-frame-20261026/compound-checkpoint.json`
supersedes the foundation counts with 40 focused tests, 457 library passes
(three ignored) and 14 independent standalone raster/text checks. Compound styles,
generated invisible-frame replay and tagged-content output are implemented.
All-state document loading/publication and actual source-absence admission remain
unfinished; `.superpowers/sdd/2026-10-26-form-frame/next-step.md` records interfaces.

**Files:** Create `src-tauri/src/form_frame.rs`, `src-tauri/src/form_frame_render.rs`, `src-tauri/src/form_frame_tests.rs`; extend Task 1 parser/tests for absent and compound frames.

**Interfaces:** `FrameStyle {Solid,Dashed,Underline,Beveled,Inset}`; `FrameColorChange {Transparent,Rgb(String)}`; `FormFrameChanges {width: Option<f32>,style: Option<FrameStyle>,border: Option<FrameColorChange>,background: Option<FrameColorChange>}`. `FormFrameAppearance` reports `style_key`, physical `width`, style, original colors and supported styles. `SetFormFrameRequest` carries annotation index, expected bounds/value/style key and changes. Internal `inspect(bytes,page,index)` and `change(bytes,page,index,changes)` operate in raw units; worker performs physical conversion.

- [x] Write failing metadata tests for BS/Border precedence, zero/fractional/overflow widths, rounded corners, gray/CMYK retention and unknown styles. Add per-style renderer tests asserting outer geometry, paint colors and exact retained text/decorative operators.
- [x] Implement all-state discovery and bounded copy-on-write programs, source fingerprint, metadata validation and style generation. Change only requested settings. Preserve unchanged palette components. Add absence proof, canonical beveled/inset edge proof and pressed-palette validation.
- [x] Add failures for unsafe missing AP/global NeedAppearances, conflicting state geometries/palettes, inherited resource ambiguity, malformed state dictionaries, aliases and aggregate budgets. Add repeated edits and transparent restoration without accumulating dead frame paint.
- [x] Run `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib form_frame`; confirm expected passes and record task-done evidence.

## Task 3: Native immutable occurrence transactions

**Files:** Create `src-tauri/src/form_frame_engine.rs`, `src-tauri/tests/form_frame.rs`; modify `src-tauri/src/engine.rs`, `src-tauri/src/app.rs`, `src-tauri/src/lib.rs`.

**Interfaces:** `Engine::inspect_form_frame(source_id,page_index,annotation_index) -> EngineResult<FormFrameAppearance>`; `Engine::set_form_frame_occurrence(source_id,occurrences,selected_occurrence,request) -> EngineResult<PageContentReplacement>`. Tauri commands `inspect_form_frame` / `set_form_frame_occurrence` follow existing text appearance command argument names.

- [x] Write public failing cases for five field kinds, stale value/bounds/style, content/annotation permissions, linked widgets and duplicated occurrences. Assert only selected frame changes while values/defaults/actions/other frames remain.
- [x] Implement existing edit permission checks, source validation before page copying, raw/physical conversion including UserUnit 1.7 and quarter turns, occurrence isolation, bounded after-model checks and immutable registration with original policy/path.
- [x] Add public raster/operation witnesses for outside-widget paint, labels/artwork, all states, zero/transparent frames, clear/refill, no-op fill, saved export/reopen, subsequent text style and resize edits. Keep refusals honest if an existing subsequent editor cannot prove fidelity.
- [x] Run focused `--test form_frame` plus related fill/authoring/appearance/positioning/acroform/protection suites. Record terminal counts, not inferred totals.

## Task 4: Border and background preview editor

**Files:** Create `src/editor/form-frame.ts`, `src/components/FormFrameDialog.tsx`, `tests/form-frame.test.ts`, `tests/form-frame-dialog.test.tsx`; modify `src/editor/adapter.ts`, `src/components/FormFillEditor.tsx`, `src/components/FormFillDialog.tsx`, `src/App.tsx` and existing form app tests.

**Interfaces:** Optional adapter `inspectFormFrame` / `setFormFrame` wrapping Task 3. Dialog props follow `FormTextAppearanceDialog` (`adapter,document,page,field,sourceIds,onApply,onCancel`). `onFrameAppearance` callback opens it from both forms panels. Typed draft-to-changes validation leaves Original controls absent from the request.

- [x] Write failing draft tests for 0–32 finite widths, empty transient input, original/transparent/RGB color modes, per-shape style options and reset. Write lifecycle tests for stale previews, adapter replacement, borrowed sources, render failure, Cancel/unmount and Apply once.
- [x] Implement the controls and existing composed-page preview ownership pattern. Integrate permission gating and occurrence replacement with Undo/Redo and recovery. Keep property names aligned with native serde.
- [x] Run focused Vitest tests, full frontend suite and TypeScript/Vite build; preserve failures. Mark complete only on terminal passing evidence.

## Task 5: Review and Windows publication acceptance

**Files:** New evidence under `artifacts/form-frame-20261026/`; update README, product-quality-loop and this plan/ledger only after gates complete.

- [x] Freeze changed source/test hashes and request one scoped fresh-context review with spec/plan/rulings and exact evidence limits. Correct actionable findings inline with failing reproductions; do not loop reviewers.
- [x] Run final relevant native/frontend/build gates with stable fingerprints. Stage a new uniquely named executable and preserve all old apps.
- [x] Run owned Windows text/choice/check/radio Preview/Apply/Cancel, Undo/Redo, save/reopen and copied-profile recovery. Include fractional width, transparency, gray/CMYK originals, rotation/UserUnit and labeled radios. Check actual later filling.
- [x] Independently audit saved PDFs with pypdf/MuPDF; assert values/defaults, frame geometry/colors, invariant foreground/outside pixels and unchanged inputs. Retain unsupported cases as explicit limits.
- [x] Write a checkpoint binding exact sources, exe/runtime and terminal reports. Update user-facing executable link only after success. Keep overall quality goal active and parity unclaimed.
