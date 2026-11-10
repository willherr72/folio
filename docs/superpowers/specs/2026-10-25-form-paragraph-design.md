# Paragraph editing in reusable PDF objects

## Intent and authorization

Extend the existing paragraph workflow to text inside nested/reused Form XObjects.
The user authorized autonomous development toward dependable everyday PDF editing;
the architecture below is the implementer's choice, not a newly requested user
preference. Preserve source files, existing dirty work and prior checkpoints.

The current UI excludes runs with objectPath from paragraph editing. Single-run
form edits already clone one invocation's resource chain. Paragraph editing has
source-font layout, fitting, mixed styles, preview, full-source occurrence-safe
publication, saved logical memory and exact re-edit verification, but operates on
page-level text. Reuse those capabilities rather than create another layout model.

## Approaches

1. Reflow inside cloned leaf streams: retains form structure, but needs inverse
   coordinate layout, nested clip-aware fitting, local replay memory and resource
   mapping in a second publication path. More independent fidelity mechanisms.
2. Flatten every form on the page: easy to feed existing paragraph code but changes
   unrelated structure/resources and broadens renderer risk. Rejected.
3. Selectively promote the clicked top-level invocation to ordinary page operators
   in a private candidate, then use existing paragraph transactions. Chosen: other
   top-level invocations retain their Do operators and original resource bindings;
   promoted descendants retain their original geometry/text/state. Only Apply
   publishes the final edited source, as one workspace undo operation.

## Projection contract

- Reuse form_graph preflight, Model parsing, bounded resource copying, inherited
  state/encoding/font verification, affine matrices and BBox admission. Initial
  eligibility is the existing validated text-only Form graph; unsupported graphics,
  optional/tagged/transparency content remain explicit refusals. This does not
  broaden the renderer or existing paragraph orientation/script capabilities.
- Bind the full original objectPath to a native verified text occurrence. Its
  first element chooses the single top-level Do to promote. Prefix matching must
  never accidentally promote another invocation of the same stream.
- Record each root invocation's exact flattened operator range while Model builds
  its existing private validation page. Replace just the chosen root Do with its
  balanced q/.../Q range. Original root operators before/after it stay verbatim.
- Install only the range's referenced font aliases, avoiding every existing root
  alias; rewrite only those range-local Tf names. Retain all original resource
  dictionaries/categories, XObjects and sibling aliases. Inherited root Tf and
  other graphics/text state continue to apply. Root text after the invocation
  must see precisely the original restored state.
- Every original glyph must already lie within every ancestor BBox, as existing
  form admission proves. Promotion removes implicit form clipping only after
  proving no visible content changes. New paragraph area may subsequently extend
  beyond the old BBox through the explicit page-space preview, subject to existing
  page-edge/neighbor/annotation protection. BBoxes are not retained as invisible
  post-promotion edit limits. Original unsupported clipping stays refused.
- Reopen the promoted bytes and require identical geometry, complete glyphs and
  exact baseline raster, not the private flat-validation antialias tolerance.
  Verify path-to-page-index mapping against all promoted native runs and original
  verified form snapshots. Anchor must map uniquely. Keep all fields/source bytes
  immutable until complete-source publication.
- Mapping scope for manual selection is the promoted top-level occurrence's text.
  It may contain several nested children. Only selected pieces may be rewritten;
  suggestions must not absorb original page text outside that scope. Native
  inspect/fit/preview reject indices outside it on every request.
- Bound decode bytes (16 MiB), operators (100,000), depth (12), verified form runs
  (5,000), source bytes (512 MiB) and all direct resource copies through existing
  CopyBudget. No persistent cache or unbounded intermediate ownership.

## Native and frontend integration

A worker-owned helper creates the promoted source, copies the source's single-page
policy/original path/legacy flag, invokes a callback and releases the source on
success/error. No private source ID escapes. Full source permission checks run
before projection; caller-supplied paths and indices provide no authority.

Add an occurrence-aware native facade: list promoted runs plus anchorIndex;
inspect/suggest styled paragraph in that scope; fit; preview with original source,
page occurrence mapping, path and the existing StyledParagraphLayoutRequest.
Complete-source publication remains publish_text_occurrence, including duplicates,
annotations, widgets, links/bookmarks and failed/abandoned-reply cleanup. Requests
use displayed physical points; convert UserUnit exactly once and restore unchanged
source f32 values using freshly inspected promoted selection, as existing APIs do.

Frontend binds this facade to a per-dialog adapter and mapped anchor. Preparation
is read-only and uses the existing busy/stale-selection safeguards. Existing
StyledParagraphDialog supplies typing, style changes, Fit, Preview and Apply.
Candidate source ownership stays in paragraph-transaction.ts. Cancel releases
candidates; Apply retains only the final source; Undo returns to the original
form structure. After save/reopen, edited paragraph fragments are ordinary page
text with existing verified paragraph memory. Unselected promoted text remains
editable as normal page text; untouched invocations retain their form paths.

The UI exposes Edit paragraph only when the full form-paragraph adapter contract
is present. Unsupported forms produce the native reason, not an empty modal or a
claimed success. This is a paragraph editing extension, not form-field authoring.

## Acceptance gates

Native projection: nested/shared calls; root font-name collisions; inherited Tf,
color and spacing; root text before/after; noncommuting transforms; page rotations,
nonzero crop and UserUnit1/1.7; copied-input and untouched invocation equality.
Refusal: stale path/index/text, clipped text, malformed/cyclic graphs, protected
source rights and bounded-resource failures; projection cleanup on every outcome.

Native transaction: multiple mixed/uniform runs, real reflow and fitting, isolated
shared occurrence and duplicate page, annotated/form-bearing complete sources,
font substitution, unchanged replay and save/reopen. Existing unsupported graphics
and scripts remain documented gaps, not proof of full producer compatibility.

UI: mapped anchor, scope-limited manual selection, busy/cancel/stale response,
whole-source identity, 1-step Undo/Redo, saved re-edit and malformed candidate
ownership. Windows synthetic nested/shared fixture and independent MuPDF/pypdf
extraction/pixel/resource checks. No private quiz data in review fixtures.

## Builder discretion

Choose localized module names and helper factoring consistent with the repo.
Avoid changing legacy API semantics. Do not commit, merge or release. Maintain
plans/evidence and one final scoped correctness review with test-verified fixes.
The feature is incomplete until native, frontend and desktop gates pass.
