# Saved comment page controls implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline; one focused fresh review at completion.

**Goal:** Read, select and move saved PDF notes directly on the page without losing native fidelity.
**Architecture:** A dedicated SVG interaction layer renders hit targets and local draft bounds; App commits through the existing immutable source replacement. Native metadata supplies page visibility and unchanged Contents stays untouched.
**Tech Stack:** React/TypeScript/SVG, Rust/lopdf/PDFium, Vitest, Windows WebView acceptance.
**Spec:** docs/superpowers/specs/2026-10-23-native-comment-page-controls-design.md

## Global constraints
Preserve original PDFs, earlier work and portable packages. No dependencies, version bump, commits or publication. Existing native permission and stale-source guards stay authoritative. User opted out of routine approvals.

## Review focus
Hidden annotations cannot create invisible targets. Repeated keyboard steps must remain one pending operation. Source/zoom/tool changes cancel gestures. Native failures cannot orphan sources or commit stale state. Rotation and page boundaries must match displayed movement.

### Task 1: Native visibility and movement fidelity
- [x] Add failing comment-fidelity tests for page visibility flags and exact unchanged Contents (including missing Contents).
- [x] Add onPage metadata and avoid rewriting Contents when decoded text is unchanged.
- [x] Run affected native comment/fidelity tests.

### Task 2: Page interaction and App wiring
- [x] Add failing NativeCommentLayer tests for rotated pointer/keyboard movement, bounds, cancellation, permission and visibility guards.
- [x] Add NativeCommentLayer.tsx and CSS; wire PageView and DocumentViewport to native notes, editor and move callbacks.
- [x] Refactor App's native mutation to accept an explicit captured request; page moves use the same guarded source lifecycle and report failures.
- [x] Add App regressions for page moves, shared-source undo and late-result cleanup; run affected tests and production build.

### Task 3: Review and desktop verification
- [x] Obtain one fresh focused review; address Important/Critical findings with regressions in one pass.
- [x] Run full frontend checks and relevant native checks; build a copied debug acceptance executable.
- [x] Exercise actual Windows pointer/keyboard movement, undo/redo/save/reopen and independent PDF fidelity checks. Inspect screenshots.
- [x] Record evidence, limitations and unchanged earlier artifacts; stop owned GUI/build processes.

Evidence: artifacts/comment-page-verification.json. Source controls and numeric saved geometry are verified. A distinct remaining workspace-rotation preview issue for NoRotate icons is recorded in artifacts/comment-page-final-desktop/remaining-rotation-preview-gap.json and is the next cycle; no broad parity claim.
