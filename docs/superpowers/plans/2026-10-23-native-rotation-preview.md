# Native rotation preview implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline, with one focused review.

**Goal:** Match native annotation appearance and page controls before/after saving workspace rotation.
**Architecture:** Render the final native page rotation, counter-transform its raster within canonical SVG interaction space, and derive NoRotate hit bounds from the final display anchor.
**Tech Stack:** React/SVG, TypeScript, existing Rust/PDFium composition, Vitest and Windows acceptance.
**Spec:** docs/superpowers/specs/2026-10-23-native-rotation-preview-design.md

## Global constraints
No dependencies, commits, publishing, version bump or prior artifact overwrite. Keep native edit coordinates, lazy rendering, URL cleanup, preview error handling and source immutability.

## Review focus
Avoid double rotation/stretching; stale old-angle frames; demo fallback regressions; hit targets diverging from the native raster; regressions in image/draw/signature drafts or thumbnails.

### Task 1: Metadata and hit geometry
- [x] Add a failing serialized NoRotate assertion and quarter-turn hit-box tests.
- [x] Expose noRotate metadata and add a final-display hit geometry helper; keep editable bounds unchanged.
- [x] Test normal/NoRotate bounds and movement/clamping across rotations.

### Task 2: Rotation-aware native raster
- [x] Add failing tests for rotated plain/image pages, thumbnails, raster dimensions, counter-transform and stale angle disposal.
- [x] Send workspace rotation through useComposedPage, retain single in-flight coalescing, and hide stale geometry frames.
- [x] Wire PageView and thumbnails to native rotation and counter-transform only raster images; retain canonical SVG/text interactions.
- [x] Run affected tests and production build.

### Task 3: Verify and record
- [x] One focused review; fix material findings with regressions.
- [x] Run full frontend and affected native checks, build a separate debug executable.
- [x] Actual Windows pointer/keyboard, rotation and save/reopen acceptance; native raster/independent PDF comparison.
- [x] Record resolved gap and remaining quality work, preserve earlier artifacts and stop owned processes.

## Acceptance-discovered correction
The Windows move/rotation sequence exposed a dangling Popup.Parent after PDFium copies one page from an edited multipage source. Preserve the validated unique forward Text.Popup ownership when repairing that copied backlink. Add native preview/export/reopen regressions and retain the generated 4,431-byte synthetic fixture at tests/fixtures/native-comments-rotation.pdf (from verify-comment-page-controls.py --generate; no user PDF content). Verify the broader composition/form suites and rerun the actual Windows acceptance before claiming completion.
