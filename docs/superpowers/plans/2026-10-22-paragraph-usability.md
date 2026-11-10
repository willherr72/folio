# Paragraph Usability Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline. The user authorized autonomous execution without routine permission checkpoints.

**Goal:** Select and review paragraph edits visually with a readable native preview.

**Architecture:** A focused ParagraphPagePreview renders and owns the original-page
image URL, displays synchronized source targets/area, and switches to the existing
dialog-owned candidate URL for comparison. App supplies page dimensions.

**Tech Stack:** React/TypeScript, existing FolioAdapter native rendering, Vitest.

**Spec:** ../specs/2026-10-22-paragraph-usability-design.md

## Global Constraints

- Keep immutable source and candidate ownership contracts unchanged.
- No new product dependency, schema change, source-PDF overwrite or app restart.
- Existing worktree contains prior user-authorized changes; preserve them.

## Review Focus

- Late raster promises must revoke URLs without closing source documents.
- Zoom/comparison must preserve the exact candidate and Apply eligibility.
- Source/candidate dimensions must match text-target geometry.
- Source-render failure must leave the editing workflow usable.
- Small viewports and keyboard users must reach Preview, Apply and Cancel.

## Task 1: Visual page preview

- [x] Add `tests/paragraph-page-preview.test.tsx`: original render, source targets,
  read-only editing state, area geometry, zoom/comparison, late URL cleanup and errors.
- [x] Run targeted tests and observe failure before implementation.
- [x] Add `src/components/ParagraphPagePreview.tsx` and scoped CSS.
- [x] Run targeted tests; expected all pass.

## Task 2: Integrate paragraph workflow

- [x] Extend dialog tests for page selection, comparison preserving candidate,
  pending original-render cleanup and footer actions.
- [x] Add pageWidth/pageHeight props to TextParagraphDialog, pass App dimensions,
  integrate the preview, move Preview to footer and update affected fixtures/tests.
- [x] Run dialog/App tests, full frontend suite and build; expected all pass.

## Task 3: Acceptance and review

- [x] Exercise selection, checking, zoom, original/edited comparison, overflow and
  Apply in a browser fixture using the native quiz output.
- [x] Request one fresh review, fix material findings with regression tests.
- [x] Build the executable checkpoint and update docs/product-quality-loop.md.

No commit/push/merge is required for this local checkpoint.
