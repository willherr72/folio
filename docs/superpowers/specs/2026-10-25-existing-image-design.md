# Existing PDF image replacement

## Intent and authority

Folio can insert pictures but cannot replace a picture already in a PDF. Add
selection, PNG/JPEG replacement, explicit comparison/Apply, and one-step Undo.
This is the next capability in the user's authorized autonomous quality loop.
The choices below are the implementer's decisions, not new user requirements.
No commits, release, or Adobe/Foxit parity claim. Preserve the accepted executable.

## Approaches

1. Cover the original with an overlay: easy, but breaks transparency, clipping,
   paint order and later editing. Rejected.
2. Mutate a PDFium image and regenerate content: simpler discovery, but resource
   sharing and unrelated operator regeneration complicate fidelity. Rejected.
3. Clone the selected raw `Do` invocation/resource chain, preserving surrounding
   operators and form dictionaries. Chosen; use the existing private projection
   and full-source occurrence publication machinery to preserve document context.

## User workflow

Add an **Edit images** tool, distinct from Insert image. Show keyboard-focusable
image quadrilaterals on the page. Clicking or Enter/Space opens Replace image.
Choose a PNG/JPEG, choose Fit (default), Fill or Stretch, Preview, compare Original
and Edited with zoom, then Apply or Cancel. Apply commits one workspace history
step. Existing crop, rotation, overlays, annotations, fields, bookmarks, siblings,
other pages and duplicated occurrences retain their behavior. Errors retain the
source and draft. Changing asset or fit mode invalidates an earlier preview.

Selection polygons must intersect the visible crop; clip the polygon, not just
its bounding box. Fully hidden images have no pointer/keyboard target. No new
move/crop authoring, inline-image editing, redaction, vector replacement, external
image fetching or image extraction is included. Replacing is not secure erasure.

## Native model and occurrence identity

Create `existing_image_model.rs` for raw parsing and selective rewrite,
`existing_image_edit.rs` for worker/facade and proof. Use strict content parsing,
raw operator ordinals from page through nested Forms, and f64 CTM composition.
The model returns path, four transformed unit-square corners and pixel dimensions.
Apply Form Matrix before its content. q/Q restores CTM. Finite nondegenerate affine
transforms include rotation, reflection, shear and nonuniform scale; reject invalid
matrices. Geometry becomes displayed physical points exactly once at the facade.

Expose `list_existing_images(source,page)` and
`preview_image_replacement_occurrence(source,occurrences,selected,path,selectionId,
assetId,fitMode) -> PageContentReplacement`. Bind selectionId to immutable original
source ID, original page, raw path and checked model identity; rediscover/validate
native geometry, never trust client coordinates. Listing and preview use the same
deterministic private page projection. Return no private source IDs.

Preserve raw operators except the chosen Do resource name. Clone each containing
Form's dictionary, content and explicit resource dictionary along the selected
path, preserving Matrix/BBox/Group and other dictionary values. Other invocations
continue to reference their original resources. Source image masks/Decode/color
belong to the replaced image; surrounding clipping, opacity and blend remain.

## Resource lifetime and bounded admission

Alias removal is permitted only after proving the relevant resource scope has no
hidden inherited XObject consumers. Initial bounded admission requires explicit
Resources on invoked Forms; rejects Type3 fonts, nonempty Pattern dictionaries,
ExtGState soft-mask groups and unsupported XObject types in traversed scopes.
Ordinary text/fonts, paths, clipping, shading and ca/CA/BM graphics states remain
supported. Reject malformed/cyclic references and missing Do targets. Conservative
refusal is a compatibility limit, not a claim that these PDFs are invalid.

Within admitted scopes, count local Do references. A unique alias can be rebound;
for a shared alias allocate a fresh name and change only selected Do. Prune only
unreachable indirect objects after rewrite; never globally delete unused resource
names. Full-source publication already captures annotation appearance contexts
before changing page resources. Test this dependency explicitly.

Prepared incoming assets use image_assets/image_pdf, including EXIF orientation,
PNG alpha and color profiles. Wrap prepared image in a unit-square Form with an
explicit unit clip and centered fitting transform. To avoid repeated wrapper
growth, recognize a replacement frame only by a complete exact structural grammar:
unit BBox, no extra semantic dictionary keys/Matrix/Group, explicit sole-XObject
Resources, q / unit re W n / positive diagonal centered cm / Do / Q, referencing
the prepared q cm Do Q unit Form with one image. A marker alone is never authority.
Treat that complete frame as a single selectable image and replace it whole.
Anything not exactly recognized is traversed normally. Repeated saved/reopened
replacement must have bounded object count, resource count, depth and file size.

For frame axis lengths Lx=hypot(a,b), Ly=hypot(c,d), incoming oriented ratio w/h,
let r=(w/h)/(Lx/Ly). Fit: (sx,sy)=(1,1/r) if r>1 else (r,1).
Fill: (r,1) if r>1 else (1,1/r). Stretch: (1,1).
Center with tx=(1-sx)/2, ty=(1-sy)/2. Preserve outer affine transform.

Budgets before copies/decoding: existing 512 MiB source limit and form_graph
preflight; 16 MiB total decoded selected-page model content; 100000 total expanded
operations, depth12, 5000 image occurrences, q stack256, direct CopyBudget16MiB /
100000 values. Reference resolution depth64. Images max32768 each dimension and
32 million pixels each, max80 million aggregate unique referenced source image
pixels on selected page (including masks). Dimensions checked without decoding.
Incoming registry limits64MiB encoded/32MP and prepared cache128MiB remain. Prepared
installation is charged separately from direct resource cloning; no unbounded
prepared copies. Bound coordinate magnitude to 1e8 and reject near-zero axes or
determinant. Do not apply form_text's scale100 limit to image placement matrices.

## Fidelity and ownership

Require editContent and existing duplicate-page assemble policy; authorize imported
asset for the destination policy image origin. Use with_text_projection and
publish_text_occurrence internally without unrelated renaming. Register candidate
with policy/original path intact. Existing publication checks retain full-source
annotations/fields/bookmarks and duplicate-page maps.

Before replacement, a clone/no-op rewrite must preserve complete glyph snapshots
(including generated separators), page geometry and exact raster. After replacement,
glyph snapshots and page geometry must remain equal; pixel changes are restricted
to the selected frame quadrilateral plus at most two raster pixels of antialiasing.
This bound is a runtime guard, not the only fidelity proof. Structural chain isolation
plus independent-reader regression prove untouched siblings and later painted text.
Never accept a candidate that fails proof. Drop temporary registrations on every
error and abandoned worker reply.

Frontend uses source/page/history snapshot guards, existing replacementPages and
commitPageReplacement. Modal owns image lease and candidate; stale/late results
are closed unless already workspace-owned. Never close a borrowed source from a
malformed response. Candidate embeds bytes; dialog image lease can release after
apply. Apply transfers candidate ownership once; cancel/unmount releases it once.

## Acceptance

Native: shared images/forms, nested affine transforms, clip/opacity/paint order,
all fit modes, alpha/EXIF, malformed and bounded inputs, stale path/ID, inherited
resource rejection, annotation appearance resources, protected origins, duplicate
page mapping, repeated replacement and unchanged source bytes. Combined CropBox,
intrinsic rotation and UserUnit1.7 save/reopen. Check glyph separators unchanged.

Frontend: clipped polygon selection/keyboard; modes invalidate preview; one-step
Apply/Undo/Redo; cross-tab/stale/unmount cleanup; borrowed IDs; late imports;
failed rendering/publication; narrow layout. Focused tests then full frontend/build.

One scoped final correctness review and one verified correction pass. Fresh debug
executable and isolated Windows acceptance: choose/preview/compare/apply/Undo/Redo,
save/reopen/re-replace. Independent MuPDF/pypdf inspect saved resources/text and
unchanged sibling/page pixels. Preserve every failure, original input and prior exe.
Printing, broader producer/performance/a11y coverage and release remain explicit
limits. Product milestone is incomplete until native, desktop and saved-file gates pass.
