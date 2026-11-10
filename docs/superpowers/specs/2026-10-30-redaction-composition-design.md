# Owned edited-document snapshots for text removal

## Intent

Prepare the actual edited document before marking text: current source versions,
ordered/duplicated page occurrences, overlays, crop/rotation and bookmarks. Reuse
the Save compositor, not a second renderer or the original PDF. This is the next
part of visible marking/preview/Apply/save-copy; the UI remains a following step.
The user authorized autonomous execution; these implementation choices are ours.

## Approaches

- Submit original source IDs and project marks back through workspace changes:
  insufficient for overlays or duplicate occurrences and can bypass unsaved edits.
- Compose an owned working copy (chosen): one immutable source represents exactly
  the document being marked. Use existing Save validation, preservation and native
  rendering. Input references remain bound until Apply.
- Flatten every page to images: loses editable text and cannot provide structural
  text removal; rejected.

## Native interface and ownership

Add PdfEngine::prepare_text_redaction_source(owner, request:ExportRequest,
bookmarks:Option<Vec<DocumentBookmark>>)->TextRedactionSource. The response contains
document:DocumentInfo and omittedMetadata:Vec<&'static str>. Public request coordinates
are physical points; reuse export_in_raw and capture_export_resources. Require
flatten=false,1..1000pages, at most10000total overlays before capturing resources.
Validate request/identifiers and edit/copy authorization for every unique source;
protected/print-only sources remain unsupported by this text-removal workflow.
Bounded-load each unique input before calling the general compositor; do not
bypass the previous aggregate initial-parser limits through this new route.
Do not admit an unadopted candidate or another prepared source as an input.

Inside the existing serial worker, temporarily use the captured font registry as
other export jobs do, restore it on every Result path, and retain image leases
through composition. Call compose_pdf with CompositionIntent::Save and supplied
bookmarks (None means normal inherited navigation; Some is an explicit current
snapshot). No file chooser, temporary PDF file, or disk publication.

The native preview store gains prepared sources keyed by source ID: owner, owned byte
allocation, input source IDs and their immutable Arc byte allocations, creation
Instant. At most4prepared sources in addition to existing4pending previews. A
prepared source is at most64MiB; prepared and pending output bytes share the
existing128MiB retained budget. Check capacity before composition and final byte
budget before source registration. Existing compositor allocation/native memory
limits remain; this is a retained-byte limit, not exact peak RSS certification.

Register a complete generated source without importing annotations out of its
native page representation. Return correct physical page sizes and bookmarks
read from the composed PDF. Retain first input's original path on the working
copy, as current generated edits do. The future frontend session must keep ALL
input source IDs until tab close (including undo history), so existing global
save destination guards continue to protect every original input.

Preparation does not additionally strip unsupported graphics, fonts, annotations,
forms or overlays merely to pass strict text-removal admission. It may return a
working copy that the current text-removal kernel later refuses.

Builder-check resolution (implementer's choice): existing Save page-import
composition can omit trailer Info/ID and catalog metadata, navigation settings,
actions or attachment trees. Refusing all such inputs would exclude many ordinary
producer PDFs. Record potential omission categories from bounded-parsed inputs
before composition, then report them only when the compositor actually chooses
the page-import route. Refactor its shared internal implementation to return
(bytes, retained_complete_source); ordinary Save behavior remains unchanged.
Catalog Type/Pages/AcroForm/Outlines are handled by existing preservation and are
not counted as omitted; other catalog entries produce the generic category
"catalog metadata, navigation settings and attachments (Save composition)".
Trailer fields except Root/Size/Prev/XRefStm produce "document information and
identifiers (Save composition)". These are category receipts, not claims that
every item in a category was present. Complete-source retention reports none.
Prepared records retain this receipt; subsequent removal previews union it with
the kernel's omissions, so information lost before inspection is still disclosed.
The visible workflow must present it before Apply and cannot claim arbitrary
document-level fidelity. No second builder review is needed for this resolution.

## Validation and cleanup

- Preview and Apply on a prepared source require the same native owner, the
  owned prepared allocation, and every original input still open with the same
  immutable byte allocation and current edit/copy rights. No caller-supplied
  provenance is trusted. An ordinary source retains its previous behavior.
- Source-close hooks invalidate working copies that depend on the closed input,
  and close their pending previews. Closing a working copy invalidates its pending
  previews. Do not close any original or successfully applied output.
- Release owner,15-minute expiry and abandoned successful Prepare replies clean
  prepared sources and any dependent pending previews. Remove records before
  calling close hooks; use owned-allocation checks so stale IDs cannot close an
  unrelated replacement. Nested prepared-source dependencies are refused, bounding
  cleanup depth. Failed preparation leaves existing sources/previews untouched.
- Existing trusted host session/window boundary gains prepare; no native owner
  reaches the renderer. Window destruction during preparation uses existing
  post-operation cleanup. Add prepare_text_removal_source command and typed
  optional adapter method. The transport name says source, not a committed edit.

## Fidelity and publication contract

Working copy pages are in the exact submitted order, each duplicate independent.
Workspace rotation/crop and overlays are already composed; future UI must not
apply them twice. Marks use working-copy page indices/physical display coordinates.
Native text removal currently rejects noncanonical page geometry and many content
classes; do not expand admission without corresponding fidelity tests.

The future Apply transaction must validate the original adapter/tab/history
snapshot, replace the document plan as one undo step, keep original sources owned,
clear baked overlays/transforms, and explain any metadata/navigation omission from
the strict rewrite before adoption. Native snapshot preparation does not grant
permission to silently discard bookmarks or other content. Save-copy must use
atomic publication and original-input overwrite checks. None of these UI gates
are claimed complete by this native preparation milestone.

## Verification

Use self-contained native fixtures to compare prepared renders to existing page
plan rendering for ordered/duplicated pages, standard text, ink, highlight/comment,
crop/rotation and forms where supported by the composer. Check actual text/page
order/bookmark targets and composition omission receipts, not only success. Prove removing text from one prepared
duplicate leaves the other unchanged. Include custom-font/image resource retention
through queued work, source closure/stale rights/wrong owner, shared byte/count
capacity, expiry/release and dropped replies. Unsupported removal must retain the
faithful prepared copy and original documents. Exercise physical-unit conversion.
Run focused native/adapter tests, full library, frontend build and one independent
final review. Capture fixture outputs and perform independent-reader comparisons
for the composed and edited examples. Preserve failed runs and original inputs.

## Limits and operating choices

No new dependencies, public area-redaction claim, executable/release claim or
commercial parity claim. Retained budgets do not prove bounded native peak memory.
Keep inherited dirty work and artifacts; use source snapshots instead of commits.
Real desktop interaction, full current-history transaction and save-copy remain
required next. Existing parser/kernel admission is not broadened in this step.
