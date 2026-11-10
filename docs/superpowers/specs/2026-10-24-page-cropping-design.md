# Reversible page cropping

## Intent and authority

Add a practical page crop workflow to Folio's page tools while preserving existing text, forms, comments, overlays, and source identity. The user authorized autonomous implementation toward everyday PDF-editor usefulness and opted out of routine design questions. The approach and scope below are assistant decisions, not claimed user choices. No parity or release approval is implied.

## Approaches

Use an optional crop rectangle on each workspace page and apply a PDF CropBox during composition. This preserves immutable sources, undo, duplicate-page independence, and shared-form ownership. Rasterizing the retained region would destroy editing and fidelity. Forking a new PDF source for every crop would fragment source identity and complicate shared widgets/bookmarks; it is rejected. The page setting fits the existing page-plan architecture, at the cost of auditing all display-coordinate consumers.

## Contract

- `PagePlan.crop?: AnnotationRect` is an axis-aligned retained rectangle in physical points in the original source's displayed coordinates, including intrinsic rotation but before workspace rotation. Page width/height and all overlay, text, form, and comment coordinates remain relative to that full source display.
- Absent crop retains the source's current visible box. The rectangle must be finite, have nonnegative origins and physical width/height at least 1 pt, and fit within source width/height. Validate using source-owned dimensions and UserUnit at the native boundary, not caller claims. Reject invalid requests; never silently clamp them into a different crop. Float roundoff tolerance may be used only for scale roundtrips at source boundaries and must not enlarge the final source box.
- Set a leaf CropBox by mapping all rectangle corners through the original PageGeometry. Preserve MediaBox, content streams, resources, original annotation/widget geometry, and source bytes. Apply after original-frame annotation/appearance composition. Workspace rotation remains independent. Existing PDF limits, protection, resource bounds, atomic publication, and font verification remain in force.
- Cropping requires `editContent` permission in the UI and native save, render, and print paths. An absent crop does not add a permission requirement. Invalid/forbidden crops fail without writing output or changing state.
- Recovery writes manifest version 4, reads versions 1–4, and continues the existing physical-unit migration only for versions below 3. Crop metadata is validated before save/restore; restored crops use physical points. Original sources and undo retention remain unchanged.

## User workflow

The selected page's actions offer **Crop page…**. A modal shows the full original visible page, including overlays and current orientation. A rectangle marks the retained area. Four numeric margins in displayed orientation and draggable edges/corners edit it; numeric inputs supply a complete keyboard alternative. Values are points, and the remaining width/height are visible. Invalid inputs disable preview/apply and explain the problem. Reset restores the source's original visible area. Cancel changes nothing. Apply creates exactly one undo step; undo/redo and duplication retain independent crops.

Show the sentence: “Cropping hides content outside the selected area; it does not remove it.” Cropping is not redaction. Reset is available throughout the current workspace; after saving and reopening, that file's CropBox is its new source visible area. Expanding to MediaBox outside the reopened source area is deferred and must not be advertised as supported.

The crop dialog offers an exact native preview of the retained result and applies only its current validated draft. Failed/stale asynchronous previews cannot authorize a different draft. Keep source registration lifetimes and current document/page checks consistent with other dialogs.

## Display and interaction

Derive the rotated retained bounding rectangle once in geometry utilities. Main canvas and thumbnails use that rectangle as their SVG viewBox and visible dimensions while retaining the full source-coordinate inner group. Full-source rasters may be clipped for interactive display; native preview/export/print must receive the crop. A cropped composite raster must never be placed as if it were a full-source raster.

Pointer mapping adds the visible display offset before inverse rotation; scrolling, zoom anchoring, bookmarks, and search navigation subtract it. Existing-text, text selection, comments, forms, overlay controls, and keyboard focus must not expose invisible hit targets. Partially visible objects can be edited in their original coordinates; retain their full content. Editors that deliberately show the full source for context must explicitly mark the retained area, rather than silently claiming that full area will print. OCR can inspect full source content, but resulting display and saved output retain the crop. Forms lists may include hidden fields as retained document content; page previews must show the correct visible area or clearly marked full-source context.

No edits automatically change the crop. Selection is cleared on applying crop so a now-hidden selected object cannot retain controls. A user may undo/reset crop to recover hidden objects. Keep objects and search data intact; filter invisible canvas targets and navigation results without modifying underlying text.

## Acceptance and constraints

Native regressions cover all intrinsic and workspace rotations, nonzero source box origins, UserUnit 0.5/1/1.7/2, exact visible dimensions, preserved contents/forms/comments, editable overlays after save/reopen, flatten/print equivalence, invalid and permission-denied inputs, and unchanged source hashes. Independent reader checks compare geometry and rendered placement against directly cropped fixture PDFs. Recovery tests cover current crops and legacy migration. Frontend tests cover margins/rotation, pointer mapping, undo/redo/duplicate, preview staleness/errors, keyboard controls, zoom/navigation, hidden targets, and cropped text editing. Desktop acceptance verifies drag/numeric crop, native preview, apply, undo/redo, rotation, save/reopen, and editing on the cropped page. Preserve the prior tested executable until a replacement passes its checks.

Use existing React/TypeScript, Rust, PDFium, and lopdf components. No new dependency or private-document upload. Do not commit, publish, or replace the original quiz. Broader batch cropping, bleed/trim boxes, margin autodetection, and expansion beyond reopened CropBox remain later work.

CropBox's display/print role is documented in the [Adobe PDPage API](https://opensource.adobe.com/dc-acrobat-sdk-docs/acrobatsdk/apireference/PD_Layer/PDPage.html). Folio's 1 pt minimum and edit-content permission policy are implementation choices; they are not claims about that API's limits.
