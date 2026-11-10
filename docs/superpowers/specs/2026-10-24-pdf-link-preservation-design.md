# Preserve PDF links through save and page operations

The replacement navigation probe demonstrated an existing export defect: copying
pages individually through PDFium drops direct cross-page Dest entries and leaves
named GoTo destinations unresolved. URI actions survive. The failure occurs in an
unmodified document, before replacement. Fix this fidelity prerequisite before
exposing the replacement workflow.

## Resulting behavior

Reconstruct Link annotation Dest, A and AA metadata from the immutable source
after native page import, using the actual source-to-output page mapping. Resolve
local named destinations to explicit destinations; preserve view mode, coordinates,
zoom/null parameters and external actions. Preserve geometry and appearance as
imported; this work changes no visible page content. Do not execute any action.

Reordering and combining PDFs must bind each source's links to its own surviving
pages. On duplicate page occurrences, self-links target that occurrence; links
to another duplicated source page target its first surviving occurrence. When
extract/delete omits a destination page, remove the local destination/action rather
than leaving a dangling or accidentally rebound link. Follow existing bookmark
action-chain remapping semantics for removed targets. External actions retain their
source bytes and parameters. Reject malformed/ambiguous mappings without publishing.

Reuse the bounded bookmark destination/action parser, including legacy Dests,
destination name trees, explicit destinations and nested GoTo actions. Bound
source traversal and reconstructed output metadata before allocation. Match source
and output Link annotation counts/order/rectangles and retain non-Link annotations.
Malformed references, cycles, unresolved named targets and over-budget metadata
must fail safely rather than silently losing navigation. Preserve original files.

Reconstructed action types are GoTo, GoToR, URI, Named and JavaScript. JavaScript
is preserved as inert data and never executed. Other action types reject before
publication: the portable value representation cannot preserve annotation, field
or optional-content object identity. This includes Hide actions even when their
target would otherwise be a simple annotation. Validate source action structure
before remapping so a malformed missing-D GoTo is not confused with an omitted
valid page. Resolve indirect Subtype values with the same bounded resolver used
for action and identity values. Broader action-reference reconstruction is pending.

## Scope and remaining replacement work

This fixes ordinary saves, reorder/duplicate/extract/combine and native previews
using a consistent source identity. It does not establish lineage between different
immutable source versions. The probe also showed split versions divide shared form
identity. Replacement UI still requires explicit cross-version mapping/ownership,
preview/apply/undo/recovery and Windows acceptance; do not claim that workflow done.

## Acceptance

Public native fixtures must cover direct/named/indirect destinations, local action
chains, remote/URI actions, source name collisions, reordered and duplicated pages,
omitted targets, null destinations, malformed/cyclic/budgeted metadata, source
preservation and protected export. Verify saved links through an independent PDF
reader and native inspection, and exact page rasters where content is unchanged.
Run affected bookmark/form/comment/crop/protection suites and a scoped review.
