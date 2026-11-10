# On-page Form Entry Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline, with one final scoped correctness review.

**Goal:** Edit supported PDF form values in an anchored on-page card sharing the existing draft and native transaction.

**Architecture:** Canvas targets publish a source-bound DOM anchor. FormFillEditor owns draft/state and portals shared value controls beside that anchor. Existing native fill/save and overview remain authoritative.

**Tech Stack:** Existing React/TypeScript, SVG, Tauri/PDFium, Vitest and packaged Windows workflow; no dependencies.

**Spec:** docs/superpowers/specs/2026-10-25-on-page-form-entry-design.md

## Constraints and review focus

- Source/page/annotation/adapter identity must match before binding a card.
- Obsolete ref cleanup, stale loads and source refresh must not use or clear a newer anchor.
- Dirty/pending values survive scroll, virtualization, Properties collapse and native failure.
- Password fields remain masked; unavailable field permissions/reasons are unchanged.
- Keyboard composition, Enter, Ctrl+Enter and Escape cannot accidentally commit/discard.
- Card stays within 12 CSS pixel window margins and follows clipped rotated targets.
- No automatic publication, source overwrites, independent draft, new native admission or commits.

## Task 1: Source-bound anchoring and placement

Files: create src/editor/form-canvas-entry.ts; extend FormPageTargets.tsx,
CanvasFormLayer.tsx, PageView.tsx, DocumentViewport.tsx and App.tsx.

Interfaces: FormCanvasAnchor carries adapter, pageId, sourceId, pageIndex,
annotationIndex, layoutKey and SVGRectElement. Callback receives identity plus
element|null; reducers reject stale clear. `placeFormEntry` returns null or screen
left/top/width/maxHeight from anchor, clip, window and content size.

- [ ] Add failing geometry/identity tests in tests/form-canvas-entry.test.ts.
- [ ] Implement intersection/clamping and anchor identity matching/clear ownership.
- [ ] Publish selected target refs; carry them through viewport props to App state.
- [ ] Run focused geometry and existing canvas-target regressions.

## Task 2: Shared draft controls and on-page card

Files: create FormFieldValueEditor.tsx and FormFieldPopover.tsx; modify
FormFillEditor.tsx and form-fill.css; add tests/form-field-popover.test.tsx and
tests/on-page-form-entry.test.tsx.

Interfaces: shared controls consume field, draft, disabled and onChange(value),
label prefix; preserve existing control labels/options. Popover consumes anchor,
focus request/revision, controlled draft/error/busy and apply/reset/dismiss actions.

- [ ] Add failing tests for on-page typing mirroring Properties, reset, apply once,
      failure/retry, same-field reopen, IME Enter, password and choice kinds.
- [ ] Extract shared controls without changing existing behavior.
- [ ] Add portal placement/observation, nonmodal keyboard behavior, focus handoff.
- [ ] Wire explicit canvas selection to popup focus and shared draft/actions.
- [ ] Run all focused form tests, then full frontend and TypeScript checks.

## Task 3: Review and desktop acceptance

- [ ] Freeze exact baseline/delta/hashes and request one scoped review; resolve
      demonstrated findings with failing-then-passing regressions.
- [ ] Run existing native form/protection preservation gates; build a fresh checkpoint.
- [ ] Test real Windows typing, all available control kinds, Apply/Reset/Undo,
      source refresh, zoom/scroll/rotation, small-window placement and keyboard focus.
- [ ] Independently compare saved values and unaffected pixels, preserve original
      hashes, inspect screenshots and stop only owned test processes.
- [ ] Record evidence, update quality checklist/README, keep the broad goal active.
