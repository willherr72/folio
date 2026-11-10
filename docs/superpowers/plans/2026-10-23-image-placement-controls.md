# Image placement controls implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline, with one fresh
> integrated review after implementation. The user authorized autonomous work;
> continue without another design or execution approval.

**Goal:** Let a person insert, position, resize, rotate and remove PNG/JPEG images
on existing PDF pages, with reliable undo, recovery and saved-copy editability.

**Architecture:** Reuse native image leases and composed page previews. A focused
placement hook owns pending imports; local pointer drafts never enter history or
recovery until committed. Geometry helpers drive both handles and numeric fields.

**Tech stack:** React/TypeScript, Tauri, Rust/PDFium and the existing image registry.

**Spec:** `../specs/2026-10-23-image-placement-design.md`.
Prerequisite: `2026-10-23-image-portable-integration.md` final checks complete.

## Constraints and decisions

- Preserve original PDFs/images and portable0.16. No publishing or PR is requested.
- Quarter-turn rotation uses the existing top-left image anchor; EXIF orientation
  is already applied inside the native unit form. Never rotate EXIF a second time.
- PNG/JPEG admission limits remain64MiB encoded,32million pixels and32768dimensions.
  Native copy provenance must remain enforced; no raw image-byte frontend API.
- Native page composition supplies pending-image, resize, ink and signature previews.
  SVG supplies interaction and handles. Use the existing one-in-flight coalescing.
- Insertion creates one undo step; a resize creates one step on pointer-up.
  Escape, pointer cancellation and stale async responses must leave no edit behind.
- Aspect ratio starts locked; its toggle is editor state, not PDF drawing metadata.
- Do not claim the image feature ready before actual Windows/portable acceptance.

## Review focus

- A late native picker response after tab/source replacement must release only its
  lease; test this in Task2, including another owner of identical bytes.
- Rotated images near each page edge must remain on-page without changing their
  aspect ratio unexpectedly; cover all image/page rotations in Task1/Task3.
- A native preview failure must remain visible and preserve the pending selection;
  test retry/cancel and stale response handling in Task2.
- Reader permissions must block edits/flattening and cross-source copying, including
  stale callbacks; Task2/Task4 reuse the native protected-image regressions.
- Large photos must not flood the worker or leave stale native previews after drag;
  profile before UI release in Task1 and repeat realistic gestures in Task4.

## Task1: Geometry and composition performance baseline

Files: `src/editor/image-geometry.ts`, `tests/image-geometry.test.ts`, focused native
probe/example under `src-tauri/examples`, artifact performance report.

- [x] Add failing tests for initial placement fit, locked/free resize, quarter-turn
  rotation and keyboard deltas. Include all rotations, small pages, boundaries,
  invalid/nonfinite values and no-op changes.
- [x] Implement pure helpers: fit an image's displayed dimensions into a page,
  resize a selected frame with optional aspect lock, and rotate while retaining
  its visual center and keeping the resulting frame within the page.
- [x] Run geometry tests. Expected: valid positive native-compatible geometry,
  consistent bounds for all quarter turns; invalid edits preserve the prior image.
- [x] Measure repeated native composition of representative PNG/JPEG photos,
  including alpha and an8MP source at page and thumbnail sizes. Record timings and
  memory observations. If preparation dominates, add a bounded immutable cache
  with tests for eviction, authorization before use and asset-release behavior.
  Expected: measured evidence, no unbounded render queue or assumed performance.

## Task2: Pending import, placement and history ownership

Files: new `src/editor/use-image-placement.ts`, `src/editor/types.ts`, `src/App.tsx`,
`src/components/DocumentViewport.tsx`, `PageView.tsx`, new image placement tests.

- [x] Write failing lifecycle/application tests: picker cancel/error, late completion
  after unmount/tab switch/source replacement, two independent identical-image owners,
  placement followed by undo/redo/delete/close and protected-reader refusal.
- [x] Add an Image action alongside existing editing tools. Hook captures target tab
  and page identity, awaits `importImageLease`, then shows a pending image only if
  the target still exists and permits annotation. Transfer the imported lease directly into workspace ownership on commit;
  release exactly once on cancel. (This avoids a second acquisition/capacity race.)
- [x] Add a page-specific native composed placement preview. Click places at the
  indicated position; Enter offers placement at the current/default page-center
  location, Escape cancels. Prevent placement from also selecting/drawing text.
- [x] Keep pending image state outside document/history/recovery. Adopt via one
  `commit` and select the new overlay. Undo/redo retains registry ownership through
  the existing all-history image hook. Native errors remain actionable in the UI.
- [x] Run focused tests and TypeScript. Expected: no leaked acquisition, phantom
  edit, double placement, stale-target mutation or missing image after undo/redo.

## Task3: Properties and resize interaction

Files: new `src/components/ImageControls.tsx` and scoped CSS, `OverlayProperties.tsx`,
`PageView.tsx`, `DocumentViewport.tsx`, geometry and interaction tests.

- [x] Add failing tests for numeric position/size, aspect lock, rotate, delete,
  pointer/keyboard resize, cancellation, focus and one-step undo at all rotations.
- [x] Implement accessible numeric fields and quarter-turn buttons. Reuse the same
  bounded geometry as drag handles; reject invalid entries without silently
  changing another dimension. Show units clearly.
- [x] Follow TextAreaControls' local-draft/cancel pattern for handles. Keep hit
  targets usable at different zoom levels; native compose the draft. Pointer-up
  commits once; pointercancel/Escape/source, zoom or tool change discards it.
- [x] Update save/flatten wording to include images and corresponding accessibility
  expectations. Existing page movement/delete actions must remain unambiguous.
- [x] Run focused properties/interaction/history tests. Expected: consistent native
  preview geometry, visible keyboard focus and one undo transaction per action.

## Task4: Integrated review, real desktop acceptance and portable checkpoint

Files: test fixtures/scripts, quality ledger and release artifacts/version files.

- [x] Run full frontend/native checks and production builds, then one fresh review
  of this plan. Reproduce and fix material findings in one test-backed pass.
- [x] Use the repository desktop acceptance workflow in an isolated copied build.
  Insert alpha PNG, rotated JPEG and a photo onto the user's quiz copy. Move,
  resize, rotate, undo/redo, save editable/flattened, close/reopen and recover after
  originals disappear. Verify originals' hashes and protected-reader behavior.
- [x] Independently inspect/render outputs with MuPDF/pypdf. Compare preview and
  output pixels/geometry, annotation order and unchanged surrounding native text.
- [x] Measure realistic drag/resize responsiveness and record remaining limitations.
- [x] Only after acceptance, prepare the next portable version while preserving0.16.
  Verify packaged startup and the image workflow from the copied package itself.
- [x] Update the parity checklist honestly. Existing-image replacement/crop,
  advanced forms/signing and other gaps remain; this is not Adobe/Foxit parity.

## Completion evidence

All four tasks completed. See artifacts/image-controls-verification.json for full
checks, one integrated review and fixes, release measurements, actual Windows
acceptance, independent PDF comparison and verified portable0.17.0-dev.1.
The original quiz and portable0.16 hashes are unchanged. No commit, publication
or PR was requested or performed. Overall Adobe/Foxit parity is not achieved.
