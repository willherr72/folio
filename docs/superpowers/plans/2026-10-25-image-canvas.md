# Existing image canvas implementation plan

> Execute inline with executing-plans/TDD, standing autonomous authority. No commits.

**Goal:** Direct on-page affine image movement, corner resize and rotation with native
preview and one-step Apply/Undo.
**Architecture:** Shared ImageReplacementDialog controller with canvas render prop;
pure affine gesture helpers; selected PageView consumes its full-page raster/handles.
**Spec:** ../specs/2026-10-25-image-canvas-design.md

## Global constraints

Preserve dirty checkout, all original inputs and prior binaries. No native API/proof
changes planned. Positive aggregate scales0.001..1000, translation abs<=1e6 physical
points. Native preview after300ms debounce; no concurrent requests per controller.
One review and verified correction pass; full quality goal remains active.

## Review focus

- Shear/reflection with opposite-corner resize and screen-direction keys (Task1/2).
- Crop/rotation/zoom changes mid-pointer capture and hidden focus targets (Task2).
- Latest draft while native preparation is pending, then cancel/unmount (Task3).
- Canvas candidate raster uses full page once with correct crop/rotation (Task3).
- Reading navigation/zoom changes selection but must preserve edit ownership (Task3/4).

## Task1: affine geometry

Files: new src/editor/existing-image-canvas.ts, tests/existing-image-canvas.test.ts;
existing-image-transform.ts widens imageMeasurements input to Pick<ExistingImage,'corners'>.
Interfaces: ImageQuad=ExistingImage['corners']; frameForMeasurements(image,page,value)
->ImageQuad; moveImageFrame(original,current,delta)->ImageQuad;
resizeImageFrame(original,current,corner:0|1|2|3,point,locked)->ImageQuad;
rotateImageFrame(frame,start,point,page,snap)->ImageQuad;
imagePointerPoint(x,y,rect,page)->ImagePoint|null; frameMeasurements(frame,page).
- [x] Add failing tests for all rotations/crop/finite mapping, reflection/shear, fixed
  opposite corner, lock, aggregate clamps, angle snapping and no-op round-trip.
- [x] Run focused Vitest red, implement geometry, run green and retain evidence.

## Task2: handles and pointer ownership

Files: new components/ExistingImageControls.tsx, existing-images.css,
tests/existing-image-controls.test.tsx.
Interface: CanvasImageView {pageId,sessionKey,image,frame,locked,enabled,raster,
onGestureActive(active),onChange(measurements)}; raster {url?,error?,loading}.
ExistingImageControls({view,page,zoom}) renders the clipped drag polygon, four visible
corner targets and an outward rotation target. Own pointer capture locally.
- [x] Add failing pointer/keyboard tests for local draft, exactly-one completion,
  competing pointer, cancel/lost capture/Escape/unmount/session/zoom changes and
  crop-excluded handles; screen-direction move/resize and snapped rotation.
- [x] Implement component plus screen-constant targets and visible focus styles.
- [x] Run focused geometry/control tests green.

## Task3: shared controller and app canvas

Files: ImageReplacementDialog.tsx, PageView.tsx, DocumentViewport.tsx, App.tsx,
existing image app/dialog tests and new canvas session tests.
Interface: optional renderCanvas(view:CanvasImageView):ReactNode, canvasZoom:number,
onLocate():void props on shared controller. Keep default modal API unchanged.
- [x] Add failing tests for nonmodal entry, no imported asset for transforms,
  debounce/latest-draft pending requests, invalidation/render failure/borrowed IDs,
  exact Reset after gesture values, one Apply/Undo/Redo and reading zoom/selection.
- [x] Add unrounded draft measurement basis; active gestures retain prior candidate,
  completion invalidates and queues native preview. Keep generation/asset cleanup.
- [x] Render editing bar around the existing viewport; use current pages identity
  guard. Block mutations/tab/tool/save while retaining reading zoom/navigation.
  Add Locate and target page number. No automatic draft loss on another image.
- [x] Pass target raster/handles into PageView, disable duplicate native rendering,
  suppress other image targets and keep target page mounted. Full-page crop/rotation
  composition remains identical to prior native raster placement.
- [x] Run focused and full frontend/typecheck. Inspect logs and fix real failures.

## Task4: acceptance

- [x] Freeze scoped sources/hashes and request final correctness review; settle
  findings with one verified correction pass, retain declined judgments/gaps.
- [x] Fresh fingerprinted debug build; reuse owned native Windows/CDP workflow.
- [x] Mouse/keyboard tests at normal/narrow widths: drag/resize/rotate, Shift lock/snap,
  crop/zoom/page rotation, cancel/lost capture, multiple adjustments/Apply/Undo/Redo,
  saved reopen, Replace/Remove compatibility, no source overwrites or pageerrors.
- [x] Independently verify saved geometry and untouched pixels/text/notes/siblings;
  record actual dependency/executable hashes, stop owned app and confirm absence.
- [x] Update product ledger/README/verification artifact; leave overall goal active.

Evidence: artifacts/image-canvas-20261025/execution.md and
checkpoint-accepted/verification.json. Final full frontend1042passed; final focused33;
Windows main+affine edge workflows and independent saved-file checks passed.
This plan's completion is only the image-canvas milestone; overall goal remains active.
