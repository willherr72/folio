# Page content replacement

Folio needs to replace outdated page artwork without deleting the destination's
comments, fields, links, bookmarks, or editor page identity. This is an ongoing
project milestone, not a claim of commercial-editor parity.

## Operation

Import one incoming page's content and resources directly into one destination
page in an immutable full-document version. Retain destination MediaBox, CropBox,
Rotate and UserUnit; center-fit the incoming visible page proportionally. Preserve
destination annotations, AcroForm, outlines and other pages. Do not import incoming
annotations, fields or bookmarks. Reject incoming Folio overlays until a deliberate
flatten/include workflow exists; never silently discard those additions.

Keep text as top-level text, preserve stream operators and resource aliases, and
omit an added crop clip only when transformed incoming crop equals the destination
viewport. Letterboxed imports need a clip and remain subject to existing editing
restrictions. Do not relax text-edit proofs as a side effect of this operation.

Incoming resource names must not rebind retained annotation appearances. Clone
appearance streams as necessary to give missing resource dictionaries their old
page context. Reject ambiguous shared annotation ownership. Retain form default
resources; reject missing form resource context. Prune unreachable imported/old
graphs so repeated replacements do not grow without bound. Clear destination
paragraph replay hints, which describe replaced content.

## Safety and scope

Use the worker's edit-content policy on destination and copy policy on incoming.
Initially require incoming sources to be unprotected; preserve destination policy,
origin and original path. Reject print-only sources. Reject tagged/optional-content
and output-intent documents, page transparency groups, and unsupported page-linked
content metadata rather than transplant document-global semantics incorrectly.
Bound resource traversal, decoded content and output; reject malformed geometry,
references, cycles and unsupported resource graph references. Originals are never
written. Failure registers no candidate.

Retained annotation appearance isolation uses a cumulative 16 MiB / 100,000
direct-value copy budget, charged before allocation, plus 64-level traversal.
Reuse reference-bound copies within one page context; preserve appearances still
used by other pages. This budget covers direct appearance/state/resource dictionaries
as well as stream payloads. It prevents small shared inputs expanding into gigabytes.

## Native milestone and follow-on interface

The first implementation plan covers the independently testable native API only.
It is not wired to IPC or offered as a completed user workflow. Follow-on work must
freeze source/destination selections for a before/after preview; apply only to the
selected workspace occurrence while keeping its PagePlan.id, rotation, crop and
overlays; commit one undo item; release stale candidates; preserve recovery and
source ownership. The dialog must explicitly explain annotation import and fitting.

## Evidence and acceptance

The initial probe (artifacts/page-replacement-probe-20261024) passed 32 independent
MuPDF comparisons but failed native comparisons. Bookmarks were incorrectly
compared with randomly generated IDs. Form wrapping eliminates top-level editable
text; direct imports with unnecessary clipping also disable editing. PDFium raster
differences between Form and direct placement are retained as evidence, not waived.

Native acceptance: same-size direct replacement remains editable where the source
already is; retained fields can still be filled; comments, links and bookmarks
retain semantic identity; unselected pages are unchanged; source files stay exact;
geometry covers all quarter turns and non-unit physical scales. Test rejection
and graph isolation, not only the happy path. Independent-reader and final Windows
acceptance remain required before UI completion.
