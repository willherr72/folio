# Direct manipulation of existing PDF images

## Intent and authority

Continue the autonomous Adobe/Foxit usefulness/quality goal from the verified
image-position checkpoint. Existing PDF images need ordinary on-page selection,
drag, corner resize and rotation, without entering a modal for every movement.
Preserve existing PDF proof/policy/ownership and one-step Apply/Undo. These are
implementer decisions under standing user authority; no routine approval, commit,
release or parity claim. Previous checkpoint and all source PDFs remain untouched.

## Approach and interaction

Use an on-page editing session. A modal-only handle surface would interrupt normal
page work; immediately committing every pointer movement would flood history and
native work. Instead clicking/Enter on an outlined image opens a nonmodal editing
bar and handles on that same document page. Default action is Position and size;
Replace and Remove, numeric fields, aspect lock, Reset, Original/Edited comparison,
explicit Preview, Apply and Cancel remain available. Preserve the standalone modal
component's existing behavior for its tests/other callers, sharing its controller.

Show a transient outline while dragging, and clearly say release previews the edit.
Do not move a rectangular screenshot of the image: it can include underlying paint
or later text and would misrepresent transparency/clipping. On release or completed
keyboard/numeric adjustment, prepare the actual native PDF preview automatically
after a short debounce (300ms). Explicit Preview remains immediate. Apply only when
the current candidate's actual page raster has rendered and no gesture is active.
Several adjustments before Apply make one history entry. Cancel discards all of
them. Escape cancels an active gesture first; otherwise closes the image session.

No document/tab/page-content mutations, save, Undo, tool switches or form edits
while this session is active. Zoom, scrolling and page navigation remain available.
Keep the target PageView mounted even offscreen, show its page number in the bar,
and provide Locate image. Clicking another image while a draft exists does not
discard it; other targets are disabled until Apply/Cancel. Session identity uses
immutable document.pages, adapter, page and image ID; selection-only reading
navigation must not invalidate it. Unexpected document/source changes invalidate
it and dispose late results without touching workspace-owned sources.

## Shared controller and page rendering

Extend ImageReplacementDialog with an optional renderCanvas(view) presentation,
or an equivalent shared controller extracted from it. Do not duplicate independent
candidate/asset ownership code. The canvas view supplies the target page ID, image
frame, lock state, full-page native raster and callbacks for gesture completion,
active-gesture status and removal. Original comparison uses the original frame;
handles are enabled only for Edited Position mode and a valid session.

Render native preview once at the main page's physical size/zoom (same 600..2400px
bound as PageView) using full uncropped page and workspace rotation; the existing
SVG viewBox clips it. Pass this raster into PageView instead of rendering a second
candidate there. Retain existing overlays/annotation composition and inverse raster
rotation. Only the selected occurrence's canvas preview changes; thumbnails and
other source occurrences stay on the current document until Apply. Full-source
replacementPages/commitPageReplacement validates final mapping as before.

The editing bar wraps on narrow windows, with optional expanded precise fields;
the page viewport remains scrollable. It is not a modal: no backdrop, inert main
page or focus trap. Keep visible keyboard focus and meaningful names on handles.
No hidden crop-excluded handle may remain focusable. Default selection receives
focus; closing returns to the Edit images tool or an available image target.

## Geometry and input

Work with the original four affine corners in intrinsic displayed physical points,
not an axis-aligned bounding box. Preserve shear and reflection. Positive scaling
retains orientation; crossing the opposite corner clamps to the smallest allowed
scale, never mirrors. Reuse native ImageTransform limits: translations within
1e6 points, scales0.001..1000 relative to the original, angle delta<=360 degrees.

Movement translates all corners by pointer displacement. Resizing fixes the
opposite corner and solves displacement in the two original image-axis vectors.
Unlocked scaling uses both solved positive ratios; aspect-locked scaling projects
the displacement onto the original corner diagonal and uses one ratio. Clamp
relative to the original session geometry, not only the current intermediate size.
Rotation uses the center and angular delta from the gesture start; Shift snaps the
displayed absolute angle to15degrees. Ignore a pointer too close to the center.
The rotation handle sits outward from the local top edge, normal to the width axis.
Handle marks/hit targets stay constant in screen pixels at all zoom levels.

Pointer mapping includes page crop and workspace rotation and allows out-of-page
positions (the numeric editor already allows them); do not reuse the clamped text
point helper. Use the current page SVG rectangle and physical visible extents.
Native clipping stays fixed, so a moved image can become clipped/invisible; Reset,
Cancel and Undo remain recovery paths. Handle hit regions must intersect the page's
visible crop before being rendered/focusable.

Own one primary pointer, capture it on the gesture element, ignore unrelated
pointers, and release on up/cancel/lost capture. Escape, zoom/crop/rotation changes,
disabled tool/session change and unmount cancel the gesture without publishing.
Clear the active gesture before releasing capture to prevent a duplicate callback.
Keyboard arrows move1pt, Shift10pt in screen directions; arrows on corner handles
resize in those directions; Left/Right on rotation handle rotate1degree, Shift15.
Do not intercept typing in numeric fields or modified application shortcuts.

The gesture draft stays local until release, so Cancel restores the prior preview
and numeric draft. On completion store unrounded measurements separately from the
three-decimal display strings; unchanged fields/Reset must not introduce drift.
An ongoing native request is never run concurrently with another for this dialog.
Changes invalidate its generation; after it settles, only the latest requested
draft is prepared. A failed preview remains an editable draft and cannot be applied.

## Acceptance and limits

Geometry tests cover all page rotations, cropped view mapping, affine/reflected
opposite-corner invariance, aspect lock, rotation snapping, no-op, boundary clamps
and finite inputs. Pointer tests cover capture/cancellation/multitouch/zoom changes,
keyboard behavior and hidden handles. Shared-controller/app tests cover debounce,
pending-result invalidation, one-step Apply/Undo/Redo, no accidental asset imports,
render failure, cancel/unmount and borrowed sources. Existing replacement/modal
tests remain green, with intentional app-entry changes documented.

One builder check and one final scoped correctness review with one verified
correction pass. Fresh frontend/typecheck/build; owned Windows normal/narrow
mouse/keyboard interaction, page zoom/crop/rotation, repeated adjustments then
Apply/Undo/Redo, Cancel/Escape and saved reopen. Independent saved-file geometry,
pixels/text/notes/siblings and source hashes. Broader producer/performance/printing,
accessibility/recovery matrices and image crop authoring remain explicit future
work. No claim of completed parity from this single capability.
