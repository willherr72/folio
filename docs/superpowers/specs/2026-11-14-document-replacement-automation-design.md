# Document replacement planning and automation

This is planned follow-on work, not a claim that it is implemented. Start only
after the corrected page-planning component is checkpointed. William's standing
authorization is to continue inline without routine design approval questions.

## Intent and approach

Make literal document Find/Replace usable through CLI and MCP without manually
assembling inspected text selections. Reuse page-wide Unicode Find, exact native
ownership, and the existing immutable batch preview/Save/Apply/Discard workflow.
The executor chooses complete document aggregation followed by automation in
one component. Per-run searching has different word and cross-object semantics;
desktop-first implementation also requires a current-workspace composition route
that has not yet been qualified. Cost of this order: desktop Replace remains
pending, while automation gains an actual consumer of the planner.

## Native document plan

Add a worker-owned document request; do not trust caller-supplied page counts or
source ranges. Require copy and edit permission before discovery, including
no-op and empty-selection requests. Scan all source pages in order and retain
every match on every page with matches. Omit empty page-detail records while
reporting the exact total/scanned page counts. A valid result is complete;
overflow, cancellation or failure returns an error, never a partial success.

Selections are optional zero-based `(page_index, match_index)` pairs referring
to original Find results. None selects all; an empty list selects none. Reject
duplicate, out-of-range and nonexistent pairs. Preserve excluded blocked rows.
The global edit list is available only when every selected match is applicable
and the complete batch fits:64 changed objects,32 changed pages,512 KiB actual native
expected/replacement/font-ID bytes. Clear all executable document and child-page
edits on a global blocker. No-op selections contribute no changed object/page.

Keep the page limits unchanged. New document bounds are1..10000 source pages,
10,000 matches/selections and4 MiB serialized native result. Check growth while
aggregating, including physical-point geometry, before retaining oversized
collections. Each page's UserUnit is applied exactly once inside the worker.
The document request owns one immutable source; every page fingerprint must
agree. Do not optimize away existing proof checks in this component.

`ReplacementPlanningControl` owns an Arc<AtomicBool> cancellation flag and an
optional progress sender. `cancel()` sets it; progress reports `(completed,total)`
at start and after each page. Check cancellation before and after every page and
before returning. Cancellation is between native page operations, not an
interruptibility guarantee inside PDFium. Progress cannot block the worker.

Expose `PdfEngine::plan_text_replacement_document(source_id,query,replacement,
options,selected,control) -> EngineResult<TextReplacementDocumentPlan>`, plus
`TextReplacementSelection {page_index,match_index}`. The document DTO carries
source identity, request, complete counts, matched page plans, global edits,
global blocker and canApply. No candidate is registered during planning.

## Automation consumer

Add strict version 1 request files containing query, replacement, Find options,
optional selections, and (only for preview) expectedPlanId. Automation selections
use one-based page numbers and zero-based match ordinals; convert once at the
boundary. Reject unknown/null members, NUL, excess sizes and duplicate pairs.
Requests are bounded at 512 KiB. No explicit font override is added in this scope.

`folio-cli plan-replacement INPUT.pdf --request REQUEST.json` and MCP
`folio_plan_replacement` return complete match/blocker information and a stable
`replacement-plan-v1-<sha256>` identity. Derive that identity from source hash,
canonical request, matched source ranges/blockers and proposed native edits;
exclude ephemeral engine source IDs. Identical reopenings must reproduce it.

`folio-cli preview-replacement INPUT.pdf --request REQUEST.json --out-dir DIR`
and MCP `folio_preview_replacement` require expectedPlanId. Replan the immutable
current input, compare identity before mutation, and reject blocked/no-change
plans without a bundle. Convert exact native edits to existing source-bound
root/nested inspection selections inside the worker; never accept native paths
from the client. Feed the resulting BatchEdit to the existing batch preview.
Keep existing version 6 manifests and Save/Apply/Discard behavior. Reuse the
session's document capabilities, credentials and worker cancellation/deadline.

Cap each final serialized CLI/MCP response at2 MiB, including the complete JSON
envelope and MCP's text/structured duplication. An otherwise valid native plan
that exceeds transport capacity fails visibly; do not truncate. MCP cancellation
kills/reaps its owned worker as today. Raw local CLI progress streaming is not
added; native progress events are available for later desktop integration.

## Qualification and remaining work

Prove multi-page roots/shared forms, last-page matches, late blockers and
overflow, whole-word/case options, selected subsets, no-op, protected permissions,
cancel-before-start and cancel-between-pages. Check32/33 changed-page and64/65 edit
boundaries without confusing scanned pages with changed pages. Independently
audit actual compiled CLI/MCP saved outputs and deliberate auditor corruptions.
Stale plan IDs, changed inputs, invalid arguments, worker cancellation and late
native fit failures must leave originals intact and no successful partial bundle.

One final component review after both tasks; one Important correction pass,
no re-review, record all declined behaviors. Preserve failures and prior packages.
No fresh frontend acceptance is implied by unchanged-input qualification.
Desktop current-workspace composition/Replace/Undo, deletion, paragraph layout,
arbitrary-font compatibility, performance, packaging and parity remain open.
