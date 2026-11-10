# Text-field resize reflow implementation plan

> Use superpowers:executing-plans inline with checkbox tracking. Standing permission
> covers implementation. One final reviewer, parent red/green corrections, no rereview.

**Goal:** Resize text fields with their original font settings and unchanged siblings.
**Architecture:** Isolated native widget probe, verified font bindings, bounded normal
appearance graft into the existing immutable positioning transaction.
**Tech stack:** Rust/lopdf/PDFium, React/TypeScript, Windows WebView2/CDP.
**Spec:** docs/superpowers/specs/2026-10-25-form-reflow-design.md

## Global constraints

Optional reflowText defaults false; UI ordinary-text default true. Move-only exact
AP preservation. Existing source admission/permissions and preview ownership retained.
Graph copy budget16MiB/100000 values/depth64. No source changes, commits or release.
One Cargo/build at a time. Existing isolated worktree; prior evidence preserved.

## Review focus

- Local AP fonts conflicting with global defaults; fallback glyph substitution.
- Shared parents, copied occurrences and unrelated APs/resources/actions.
- Rotation, UserUnit, auto-size, alignment and field metadata inheritance.
- Cyclic/oversized font/AP graphs and repeated-edit resource retention.
- Async resize-mode changes and failed native previews cannot authorize Apply.

## 1. Native text appearance reflow

Files: form_position.rs/form_position_engine.rs, new form_reflow.rs and bounded
appearance graph helper, lib.rs; tests/form_reflow.rs and positioning test literals.
Consumes PositionFormFieldRequest with optional reflow_text; returns existing
PageContentReplacement. Probe has no source-page dependencies.

- [x] Preserve existing renderer probe evidence; add failing public reflow request
  regression and observe old API refusal before product changes.
- [x] Implement isolated text probe, effective settings/font resolution, bounded
  graph import and generated-font equivalence. Graft only selected AP/N and prune.
- [x] Verify value, glyph size/line geometry, sibling appearances, local fonts,
  units/rotation, read-only/empty/auto-size, move-only and repeated resize bounds.
  Run focused and related native suites; expected supported output passes and
  unsupported graphs refuse without modifying source or publishing candidates.

## 2. Resize-mode UI

Files: editor/form-positioning.ts, FormPositionDialog.tsx; positioning tests.
Consumes optional reflowText request. Old callers still scale appearances.

- [x] Add failing mode-default and stale-preview tests. Implement text-only choice
  and clear behavior guidance; switching mode invalidates owned/in-flight preview.
- [x] Run related frontend, full suite and build. Expected no stale Apply and
  existing keyboard/pointer/Undo workflow unchanged.

## 3. Review and acceptance

- [x] Freeze cycle baseline/delta; one scoped reviewer with requirements/focus.
- [x] Reproduce and correct demonstrated important findings red-to-green.
- [x] Windows narrow/wide reflow, scale option, Undo/Redo, fill/save/reopen;
  independent font/line/pixel checks, binary/source/PDFium binding and cleanup.
- [x] Record checkpoint and remaining limits; leave broad goal active.
