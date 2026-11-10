# Combined content removal in the desktop workflow

## Outcome and authority
Extend the existing Remove text dialog into an explicit text-only or combined
text/graphics/images removal workflow. Preserve editable retained content, frozen
working-copy ownership, single-use Apply, Undo and recovery. This is phase4 of
JPEG area redaction, not completion of commercial parity. Standing autonomous
authorization opts out of design questions. Preserve originals, inherited dirty
changes, all evidence and the accepted0.30 package. Previous goal turn made
verified progress: private phase3 checkpoint3bdda1347de128cb94ea582e8a810328be4c5fd77a4fa60e198e8305cc82c171.

## Approaches and decision
Coordinator choice: extend the existing dialog/history workflow with an explicit
combined mode and new native preview/render methods. A separate independent tool
would duplicate ownership/cancel/history logic. Replacing the text-only transport
would silently change existing semantics. Keep that mode and API intact.
Use the existing React/TypeScript and Rust/PDFium stack with no new dependency.

## Native transaction
Reuse trusted window sessions and Save-composed immutable working copies. Add
ContentRedactionPreview and page summaries with removed-character, removed-path
and affected-image counts. A combined preview calls the verified area kernel with
an explicit JPEG-enabled context; default/text-only contexts retain admission.
Preparation, selection, publication, resource checks and exact source identity
checks precede candidate registration. Keep prior metadata-omission disclosures.
A blank marked page may produce no change when another marked page changes;
report only affected pages. An entirely empty removal remains a refusal.

The pending store owns combined source geometry, JPEG source masks/transforms/
old-new paint envelopes and page summaries. Include all retained geometry in the
existing32MiB shared budget with checked accounting before registration. Pending
PDF bytes remain within128MiB. Preserve source hash, candidate allocation identity,
expiry, owner isolation, close/discard/release and failed-receiver cleanup.
Only explicitly selected occurrences are removed; duplicate unselected copies
remain, and the dialog must make that scope clear.

## Renderer-bound preview
Add a session-owned bounded paired-page render. Before a candidate exists it
renders only the owned prepared source. With a preview it validates its immutable
source/candidate pair and renders both at exactly the same native fixed bitmap
size. Choose aspect-preserving dimensions from requested width64..1200, cap height
at2400 and pixels at2880000. Reject invalid pages, dimensions, unsupported source
state and oversized encoded output. This avoids trusting CSS scale or client
arithmetic as the actual PDFium viewport. Return identity/page/device metadata,
PNG bytes for source/candidate, and a PNG disclosure overlay, bounded to16MiB
aggregate encoded output. The frontend creates and revokes owned blob URLs.

Map selected glyph/path/raw-image bounds into the actual viewport with clipping
and a one-device-pixel raster fringe. JPEG disclosure uses the existing source-
bound page_influence calculation, never a newly expanded removal mask. Retain
separate removal-bound and renderer-influence layers; explain conservative bounds
and whole-path/JPEG-block expansion. Compare actual source/candidate pixels: any
changed pixel outside the independently derived influence refuses that page
preview. Pixel differences never widen the allowed region. This dynamic check
is scoped to the displayed native viewport; finite cross-reader evidence and
compatibility limits remain explicit, not a universal renderer claim.

Require a successfully rendered candidate/disclosure for every affected page
before Apply; record those pages in the pending session. Text-only Apply keeps
its existing behavior. A cancelled or expired preview cannot adopt a candidate.
Apply transfers the same candidate once; frontend history replaces the composed
pages in one transaction and retains the old source for Undo/recovery. Saving a
redacted copy must not reintroduce discarded source resources.

## Frontend
Add combined-content capability checks and an immutable request mode to the
existing workflow. Never fall back to text-only if combined support is missing.
The dialog title/copy and summaries reflect the selected mode. Combined mode
uses the paired native raster response, checks all identities/dimensions/PNG
headers/limits, and waits for all images and the disclosure to load. Stale
responses, failed images, changed tabs/history/adapters, malformed ownership,
missing bounds, expiry and unreviewed affected pages disable Apply. Numeric and
pointer marks, page navigation and keyboard behavior retain current support.
Changing marks or mode invalidates the current preview. Metadata omissions and
expansion outside drawn marks are explicit before applying. Source selections
are occurrence-specific, not a promise to erase every duplicate in the document.

## Verification and finish
TDD native tests: mixed text/path/JPEG and image-only selection; blank marked
pages; exact source preservation and replay; shared occurrence isolation;
owner/expiry/close/cancel/capacity/failed-receiver handling; device endpoints;
all-page render admission; forbidden outside-pixel corruption. Independently
inspect saved outputs, coefficients, editable text and rendered pixels.
Frontend tests: combined capability and counts, stale/malformed paired responses,
PNG identity/dimension checks, URL cleanup, image load failures, all-page review,
cancel during operations, history Undo/Redo and cross-tab ownership. Run affected
and full frontend/build gates; serialized native gates; real Windows preview,
Apply, Save/reopen, Undo/Redo, cancel and recovery with independent saved audits.
Use one fresh integrated review at the end, not a reviewer per helper. Keep the
full phase4 task open until its native, frontend and desktop gates are complete.
