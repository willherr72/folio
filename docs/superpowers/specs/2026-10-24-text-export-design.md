# Export edited document text

Add a local UTF-8 text export for current/all/range pages in workspace order,
including current native edits, added text and verified OCR reading text. The PDF
and its saved/dirty state remain unchanged. This closes a conversion workflow gap;
it does not promise Office layout reconstruction or visual/redaction equivalence.

## Approach

The coordinator selects native per-page composition and bounded extraction.
Browser text-layer concatenation would depend on virtualization and omit edits.
Exporting an entire intermediate PDF would retain more memory than necessary.
Compose one selected page at a time in memory, flatten Folio text overlays solely
for extraction, use the existing rotation-aware reader and verified OCR ordering,
then release the page. Do not create plaintext intermediate PDFs or source IDs.

Selectable content intersecting the page crop is included. Generated separators
retain reading order; normalize CR/CRLF to LF and use form-feed page separators.
UTF-8 BOM supports Windows text tools. Text under drawings or marked invisible can
still be selectable; this is not a sanitization/redaction operation. Column/table
ordering follows the PDF reader; scans need OCR first. No automatic OCR job runs.

An explicit Include form values option appends a labeled field section per page.
It includes visible intersecting supported fields, checked radio values and choice
display labels, without executing actions. Password and unsupported fields are
counted as skipped; hidden/outside-crop fields are omitted. Comments, ink and image
pixels are not page text. Warn about empty pages and skipped fields after export.

## Ownership, permissions and limits

Use a frozen page snapshot and capture font/image resource leases. Native copy
permission is required for every selected source independently of printing,
assembly, form-fill or frontend flags. A new TextExport composition intent may
flatten an authorized snapshot internally without granting content-edit rights;
existing source overlay/crop validation still applies. Never expose that temporary
PDF. Printed preparations remain forbidden. The output is intentionally plaintext.

At most 10,000 pages, 250,000 extracted characters per page and 32 MiB UTF-8 total.
Check character limits before character-vector allocation. Job registration is
bounded/window-owned; cancel between native operations and before publication.
An atomic cancellation/commit decision prevents late cancellation undoing success.
Build complete bounded text before writing; publish with same-directory temporary
and atomic replacement. Reject open source aliases/hard links, non-.txt targets and
invalid destinations. Errors/cancellation preserve any existing destination.

## User workflow and acceptance

Save menu → Export text → choose pages and form-values inclusion → native Save
dialog → progress/cancel → result with empty-page/skipped-field information. The
modal owns the operation; editing, tab changes and duplicate submits are blocked.
Closing/unmounting cancels. Export never marks PDF changes as saved.

Native tests cover edits/overlays, page order/duplicates, rotations/physical units,
crop, Unicode/OCR, fields and password omission, restricted-copy enforcement,
source/destination preservation, cancellation, empty text and budgets. Frontend
tests cover selection, stale progress/cancellation, keyboard/error/draft ownership.
Fresh Windows build and independent saved-text checks must include unsaved added
text, multiple pages, selected range, cancellation and original hash preservation.
One scoped review precedes checkpoint acceptance; retain all limits and failures.
