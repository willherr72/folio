# Interactive form page selection plan

> Execute inline with superpowers:executing-plans; one focused review and one fix pass.

**Goal:** In Fill forms, select a visible field directly on the rendered page and focus its value control, preserving native form validation and drafts.

**Architecture:** Extend listed field metadata with displayed widget bounds, visibility and NoRotate. Convert raw native annotation rectangles using the source's existing crop/rotation geometry. Render accessible SVG targets over the exact page preview, applying workspace rotation consistently. Keep the existing field list/value controls and immutable fill operation. This is a focused form-editing workspace; inline typing on the main document canvas remains a later ergonomic step, not a claimed capability.

**Tech stack:** Existing Rust/PDFium field inspection, React dialog, native page-plan preview and Windows acceptance harness.

The previous milestone is progress: portable0.20.0-dev.1 and six packaged Windows workflows verified; all owned jobs exited. Preserve that package. The user authorized autonomous work and opted out of routine design approvals. Use the current isolated linked worktree. No font writer, form definition or permission-policy change.

- [x] Native regression: inspect cropped pages at all four intrinsic rotations, normal/NoRotate widget rectangles, hidden versus read-only flags and invalid geometry without mutation. Add metadata and source-coordinate conversion only after the new test fails.
- [x] UI regression: click/keyboard selection focuses the matching value, preserves same-field dirty drafts, blocks other fields while dirty/busy, renders rotation/NoRotate correctly, omits hidden/invalid/off-page targets, and suppresses stale fields/preview across source or adapter changes. Implement a small page-target component and dialog integration.
- [x] Run focused native form suites and frontend/full build checks. One fresh scoped review for geometry, input ownership, stale async results, accessibility, permissions and shared-field identity; one fix pass.
- [x] Build a copied debug executable. In Windows select text, checkbox/radio/choice fields by page location, apply/reset/cancel, rotate/crop, undo/redo and save/reopen. Independent PDF checks must retain expected values/appearances and original bytes. Verify small-window access and process cleanup.
- [x] Record hashes and evidence, preserve0.20 package, and document remaining main-canvas/inline-editing and broader parity gaps.

Verified: `artifacts/form-page-verification.json`;694 frontend tests/101files,
23 native form tests/two binaries, TypeScript/Vite/debug build, Windows workflow
and independent PDF/raster verification pass. One review found no actionable
issues. First Windows attempt exposed an ambiguous test locator; exact matching
fixed the harness without a product change. NoRotate native cyan appearance
matches target bounds before/after workspace rotation and save/reopen. All owned
processes exited; original/prior package hashes unchanged.

Read-only fields remain selectable for their explanation. Hidden/NoView/invisible
fields have no page targets. Missing geometry retains list access. A stale raster
or stale field response must never expose clickable targets for a new source.
