# Existing image positioning and removal

## Intent, authority and approach

Continue the user's autonomous Folio quality loop. The accepted image replacement
checkpoint remains preserved. Add precise move/resize/rotate/remove for admitted
existing images, with preview, Apply, Undo/Redo and saved-file re-editing. These
implementation choices are mine under the user's standing no-permission workflow.
No commit, release or parity claim.

Reuse the bounded raw image model and full-source occurrence transaction. PDFium
regeneration risks unrelated content; overlay covering cannot remove or faithfully
move transparent images. Instead edit only the selected raw invocation, cloning
its containing Form chain. Keep original image pixels, mask, color and clipping.

## Workflow

The existing image dialog becomes Edit image, with actions Replace, Position and
size, and Remove. Retain current replacement workflow and candidate ownership.
Position offers center X/Y in physical points from the displayed full-page origin,
width/height along the image's own axes, clockwise angle, locked aspect ratio by
default, and Reset. Measurements follow workspace page rotation and preserve
existing shear/reflection. Width/height are axis lengths, not a rotated bounding
box. No automatic preview on each keystroke. Explicit Preview and Apply; any draft
or action change invalidates the preview. Removal also requires preview/Apply.
Explain that existing clipping remains, and removal is undoable, not redaction.
Controls wrap at narrow widths. Precise numeric editing is this milestone's canvas
interaction; drag/resize handles are left for a later interaction milestone.

## Native interface and geometry

Add ExistingImageEdit, a tagged union: {kind:transform, transform:ImageTransform}
or {kind:remove}. ImageTransform contains f64 deltaX/deltaY (physical points in the
source page's intrinsic displayed coordinates), scaleX/scaleY, rotationDegrees
(clockwise). All values finite; abs translation <=1e6, scales 0.001..1000, abs angle
<=360. Default transform is exactly 0,0,1,1,0. UI derives deltas from the original
unrounded measurements, so Reset is exact and default re-edit introduces no drift.
Final matrices obey existing coordinate/nondegenerate bounds. The facade divides
translation by source UserUnit exactly once, on the selected original page.

preview_image_edit_occurrence(source, occurrences, selected, path, selectionId,
edit) returns PageContentReplacement using the same ownership/policy/publication
as replacement. The original geometry is always rediscovered and identity-checked.
Transform the original axes independently by positive scales, rotate around the
original center, and translate in displayed coordinates, conjugated through the
intrinsic page rotation. Preserve shear/reflection. For original matrix C and
desired D, insert q / inverse(C)*D cm / selected Do / Q at the selected node.
Canonical immediate q cm Do Q can instead have its cm composed in place; do not
accumulate wrappers on repeated saved edits. Only exact four-operation groups
qualify, with validated numeric cm. Existing replacement frames remain selectable
as whole frames and replacement after geometry editing still uses their new frame.

Remove the selected Do (or exact enclosing q cm Do Q group). Do not remove resource
aliases blindly: retaining now-unused source resources is allowed, and removal
makes no secure-erasure promise. Siblings/shared forms stay unchanged. Existing
Form BBox and parent clips remain unchanged, so movement may clip an image; the
native preview is authoritative. Moving outside page is allowed and Undo restores
it. Do not expand clips, reorder paint, or change other content to make it visible.

## Proof, bounds and lifetime

Reuse source/model preflight, strict decoding, graph/copy/image/operator bounds,
annotation appearance context retention, exact clone proof, policy/original-path
retention and full-source duplicate mapping. Complete glyph snapshots and page
geometry stay equal. Raster differences are allowed only within the union of
original and resulting quadrilaterals plus the existing two-raster-pixel margin;
removal uses only the old quadrilateral. Rediscover output and verify expected
occurrence count and transformed geometry; structural isolation is the primary
proof for overlapping image content, not only the raster region. No-op transform
uses the exact clone path. No imported asset/lease is needed for transform/remove.
Late or invalid candidates follow existing borrowed-source and generation guards.

### Measured-ink correction from acceptance

The reflected/sheared clipped stress fixture exposed eight original PDFium image
pixels just outside the two-pixel geometric envelope (affine-diagnosis evidence).
Keep that fast geometric check. If it fails, independently remove the exact selected
invocation from before and after (after already serves as the removed version for
Remove), retaining appearance bindings. Require these two image-free witnesses to
have exactly equal complete glyphs, page geometry and raster. Thus all remaining
content must match; do not widen the geometric margin or ignore a pixel difference.
Temporary witnesses use existing graph budgets and policy, and are always discarded.
Negative tests cover unrelated paint both outside and underneath the selected image,
stale paths and cleanup. This acceptance-driven correction supersedes the geometric
envelope as the only admissible proof; no-op and output geometry checks remain.

## Acceptance

Model tests: shared page/nested invocations, shear/reflection, positive scales,
rotation/center/translation, exact no-op, deletion of only one invocation, bounded
repeated transforms, replacement then transform then replacement, invalid values.
Native tests: physical translation across all intrinsic rotations and UserUnit1.7,
shifted CropBox, duplicate page isolation, clipping/paint order, glyph preservation,
annotation appearance borrowing, save/reopen/re-edit/remove, edit rights and stale
identity. Inspect independently rendered saved outputs and original byte hashes.
Frontend: physical/workspace-rotated measurements and exact Reset; invalid/empty
numbers; locked resize; preview invalidation/action switching/pending cleanup;
Apply/Undo/Redo and no import for positioning/removal. Existing replacement tests
remain intact except intentional dialog copy changes. Full frontend/typecheck,
focused native regressions, fresh build and owned Windows acceptance at normal
and narrow size. One design check and one scoped final correctness review, then
one verified correction pass. Preserve failures, input files and earlier builds.

Remaining limits: unsupported resource structures from replacement remain;
clipped images can become invisible; no direct dragging in this milestone;
printing, broad producer/performance/a11y/recovery and parity are separate gates.
