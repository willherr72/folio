# Reversible existing-image cropping

## Intent and authority

Continue the autonomous quality goal from the verified image-canvas checkpoint.
Cropping an existing picture should work on the document page, preserve its image
pixels and unrelated PDF content, and remain adjustable/restorable after Save/reopen.
The present native engine already isolates image occurrences and proves surrounding
content preservation. Build on those transactions. No release or parity claim.
All choices below are implementer decisions under standing user authority.

## Approaches considered

Pixel replacement would destroy hidden pixels or require separate recovery metadata;
reject it for ordinary reversible cropping. Adding an unrecognized page clipping
operator would save visible output but leave later image movement/crop restoration
ambiguous. Choose a structurally recognized, isolated unit-space Form wrapper around
one logical image occurrence. It carries an ordinary PDF clip, not trusted private
metadata. The wrapper moves with that image and its clip can be changed after reopen.
Other preexisting page/Form clipping remains fixed and is never removed by Restore.

## Native model and identity

Final acceptance ruling: Restore may remove the selected crop wrapper when the
retained child's appearance fingerprint is identical in the invoking parent's color
scope and doing so cannot promote an enclosing clipping/fitting boundary. This avoids
redundant full-rectangle clips changing edge antialiasing in independent readers.
Otherwise Restore retains the local resource wrapper with a full crop. Neither path
removes a fixed parent clip. Discovery can therefore return absent crop after Restore;
post-publication still proves child resources, image pixels and all surrounding content.

Add ImageCrop {left,top,right,bottom}: retained edge coordinates in normalized image
space, x left-to-right and y top-to-bottom. Full is {0,0,1,1}. Finite bounds0..1;
retained width/height at least0.001. Normalize only endpoint-sized serialization
roundoff (f32 PDF operands); reject materially invalid/empty/nonfinite crops.
Public ExistingImage gains optional crop (normalized, never scaled by UserUnit).
Its corners continue to describe the full logical frame. Bounds may describe the
visible crop extent; consumers must use crop-aware corners for page targets.
Selection identity binds the derived crop in addition to source/path/matrix/pixels.

A crop wrapper has unit BBox, no Matrix/transparency/other behavior-bearing keys,
explicit local Resources, exactly one XObject and this complete operation sequence:
q; left (1-bottom) (right-left) (bottom-top) re; W; n; /Picture Do; Q.
The child is either a supported image XObject or a currently recognized image-fitting
frame, with exactly one resulting logical image and identical unit-frame matrix.
Do not promote arbitrary multi-image/text/paint forms or infer ownership from a name.
Keep all needed non-XObject resource categories from the source scope so named image
ColorSpaces/default color spaces retain their bindings. The selected child resource
is reused; no pixel decoding/re-encoding. CopyBudget and graph limits still apply.

Reader recognition promotes that complete wrapper to one logical occurrence with
full corners and its crop. It must not mistakenly flatten arbitrary nested crop
semantics; nested/ambiguous wrappers retain conservative traversal. Image discovery,
transform/remove/replacement and saved re-edit all operate on the promoted occurrence.
Changing a recognized crop rewrites its clip in one cloned wrapper, without nesting
one more wrapper per edit. Restore writes the full rectangle, retaining any required
local resource context. Repeated crop/restore cycles must have bounded structure.

Add ExistingImageEdit::Adjust {transform:ImageTransform,crop:ImageCrop}, preserving
existing Transform/Remove callers. Adjust applies both to the same selected occurrence
before publication; geometry/crop edits across UI modes make one Apply/Undo step.
Native validation is authoritative. Translation still converts physical units once;
crop is dimensionless. The model may add an adjustment operation that rewrites the
selected resource/clip and its isolated matrix group in one pass; preserve aliases,
parent clips, Form BBoxes and later graphics/text state. Existing replacement preserves
the selected crop around the new fitting frame. Removal removes the logical occurrence.

Publication retains full-source occurrence mapping, protected rights, annotations,
forms and original pages as today. Post-read validation checks image count, selected
full matrix/crop, and every unselected matrix/dimension/crop. Always prove image-free
contexts equal for crop changes (existing exact selected-invocation removal witnesses).
For pure crop changes, retained interior pixels (old/new crop intersection, excluding
an explicit small antialias boundary) must agree; full restore and source-color cases
are independently verified by fixtures/readers. Combined movement uses existing matrix
and surrounding-content proofs plus independent saved-output tests. Every private
registration must be discarded on error/cancel; no weakened preservation fallback.

## Geometry and UI state

Crop bounds are image-local, so skew/reflection and intrinsic/workspace rotation do
not turn the operation into an axis-aligned page crop. Mapping uses the image's two
signed axes. Raw image corners correspond to PDF unit (0,0),(1,0),(1,1),(0,1); convert
crop top/bottom using v=1-y. Page crop/zoom handling stays in physical displayed points.

The visible cropped frame is used by discovery and ordinary move/resize/rotation
handles. Numeric center/width/height likewise describe that visible frame. Convert
an edited visible frame back to the underlying full frame before constructing the
native ImageTransform, so rotation/resize are about the visible frame's center, not
the hidden original center. Preserve full-precision geometry behind display strings.

Controller owns an independent full-frame basis plus crop and visible numeric draft.
A valid visible numeric/gesture change maps to a desired full frame. Changing crop
keeps that full frame fixed and refreshes visible numeric measurements. Mode changes
between Position and size and Crop retain both drafts. Apply sends one Adjust request.
Reset position restores source full geometry while retaining the crop draft; Restore
full image resets only this crop to full. Cancel discards all pending geometry/crops.
Replace/Remove remain explicit actions; replacement retains the source's crop.

Add Crop to the on-page action selector. Crop mode displays the full frame outline,
current kept rectangle, shaded excluded area, four corner handles and a move target
for sliding the crop window. Dragging a corner keeps its opposite corner fixed,
clamps to full frame/minimum extent and preserves the initial grab offset. Window
movement preserves extent and clamps to0..1. Arrow keys follow screen directions;
Shift increases movement, with the same pointer ownership/cancellation rules as the
existing controls. Handle hit targets remain screen-sized and crop-excluded/page-
excluded handles are not focusable. Native pixels update on release, not a fake
bitmap cutout. Already-hidden pixels appear after a preview expands the crop; Restore
full image is always available for recovering that crop's hidden region.

Offer precise trim percentages for Left/Top/Right/Bottom (margins, converted to retained
edge coordinates). Error text explains invalid or too-small retained areas. Debounced
latest-draft native requests share the existing candidate/asset ownership controller;
no extra Undo per gesture. Original/Edited comparison, mutation blocking, reading
navigation/zoom, Locate, Escape/gesture cancellation and focus restoration remain.

Make the nonmodal bar compact: essential action/lock/reset/comparison/Apply/Cancel
controls stay reachable, precision fields/help live in a collapsible section, and
status uses a stable reserved region. Narrow windows must leave useful page space
and expose Apply/Cancel without scrolling a long precision form. The standalone modal
retains expanded precise fields. This is part of usable image cropping, not a separate
redesign of the application. Include a clear short note that crop hides pixels and is
not redaction; Restore does not remove preexisting PDF clipping.

## Acceptance and limits

Native model RED→GREEN: raw image and fitted frame crops; exact semantic recognition,
malformed/extra-paint wrapper nonpromotion; named ColorSpace/resource preservation;
shared aliases/nested forms; same selected occurrence; repeated crops/restore bounded;
replacement retains crop; transform/remove after save; strict finite/boundary checks.
Native integration: all rotations plus UserUnit1.7/nonzero CropBox, duplicate pages,
reflected/skewed/transparent images, notes/forms/text/overpainting and protected policy,
pure-crop retained pixels, full-restore raster identity, save/reopen/re-edit and cleanup.
Frontend geometry tests cover signed affine mapping, crop window/clamps/offset grabs,
visible-to-full transform round trips and precision. Controller/app tests cover mixed
geometry/crop drafts, debounce/stale/borrowed results, invalid fields, Restore/Reset,
one-step Undo/Redo, modal compatibility and compact controls/focus.
Fresh frontend/typecheck/build. Owned Windows normal/narrow crop/slide/keyboard,
uncrop, resize/rotate after crop, Cancel/Escape, save/reopen, Replace/Remove and source
hashes. Independent saved-file geometry, clipping, original image bytes/pixels,
untouched content and restored output. Preserve failures and dependency fingerprints.
One builder design check and one final review, then verified correction pass.
Broader producer/performance/printing/accessibility/recovery and inline-image support
remain explicit open work. Overall quality goal remains active.

## Builder-check dispositions

All seven design findings are resolved as implementer choices. Dedicated crop
recognition leaves the existing fitting recognizer strict. Direct child kind and
path identify the logical boundary; no approximate leaf matrix/producer-name trust.
Exactly matching third-party forms are recognized too: Restore removes that selected
recognized clip, not a claimed Folio provenance or arbitrary parent clipping.

Canonicalization: strict finite/range request validation with only binary arithmetic
endpoint tolerance. Writing converts the four re operands to PDF f32 values once;
expected crop metadata is derived by the same readback routine used in discovery and
identity. Readback endpoint allowance derives from operand ULPs, not display precision.
Unchanged recognized crops preserve their exact existing operators/resources, including
when only their frame moves; do not re-encode them on each no-op cycle. Absent crop
and a recognized full crop remain distinct in identity; both have full visible geometry.

Model exposes a bounded structural appearance signature for the selected logical
content: retained image/fitting child graph, image/mask dictionaries and stream bytes,
plus effective named/default ColorSpace resources. Resolve reference contents without
hashing object numbers, memoize graphs, reject cycles/depth/visit/byte excess and ignore
only incidental stream Length. Compare before/after for all Adjust operations, including
combined geometry/crop and cases with no common raster interior. Such cases report
interior comparison inapplicable; structural equality and exact image-free context
proof remain required. No empty pixel sample is represented as color proof.

UI keeps signed full-frame corners, not only five unsigned measurements. Aspect lock
and unchanged numeric strings rebase to the newly visible frame after crop updates.
Position/Crop modes share one complete draft/revision. Replace/Remove use the immutable
session source, retain adjustment draft when switching back, and explicitly say pending
position/crop changes are not included in those separate actions. Candidate readiness
is tied to action plus complete valid draft, including collapsed invalid fields.
Active gesture cancellation restores its preceding local draft; any changed session
revision still invalidates in-flight previews. Essential actions/status remain outside
collapsible precision controls. These choices add no approval gate.
