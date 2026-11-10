# PDF save progress and cancellation

Ordinary Save currently blocks editing with a busy message while composition runs.
The 10,000-page workload makes this noticeable. Add visible stage/page progress and
cooperative cancellation to **Save a copy**, **Save editable PDF**, **Flatten text,
images, and ink**, and Ctrl+S. Preserve the existing save dialog and saved-digest
semantics. Dedicated extraction/split/image/text dialogs retain their current flows;
do not stack competing focus-trapping modals.

The coordinator selected the established native job/channel pattern under the user's
autonomous-work authorization. A synthetic percentage would misrepresent variable
parsing/encryption costs; instead show named stages and page counts only where real.
Terminating the worker is inappropriate because it owns other open documents.

## Contract

- Register one save job owned by its WebView window. A 120-second pending registration
  expires at start as well as on replacement. Running jobs cannot be replaced; a
  second start or foreign-window cancellation cannot disturb the owner.
- Use a shared atomic control with a single transition from cancellable to final
  publication. Check cancellation during physical-unit normalization, between page
  imports/overlays, around preservation/serialization/encryption operations, and
  while writing the temporary file. Individual parser/PDFium calls are cooperative
  boundaries and may finish before cancellation is observed.
- Validate/authorize exactly as before. Cached source parsing, forms, links, comments,
  resources, crop, protection and byte limits retain their existing behavior.
- Write and sync a temporary in the chosen directory. Only after that may the atomic
  control grant final replacement. A cancellation accepted first returns no output,
  leaves any existing destination bytes intact, and removes only the owned temporary.
  Once replacement begins, cancellation returns false and UI says finishing. Existing
  bounded Windows rename retries and error preservation stay intact.
- Native completion returns a path only for successful publication. Cancellation or
  canceled file dialog returns null. Failure returns an error. These outcomes must
  never clear dirty state except successful editable save; flatten remains an export.
- The frontend owns one task, ignores foreign/stale progress, requests cancellation
  on unmount, and keeps editing blocked until the native result settles. Cancellation
  request failures remain visible and retryable. A late successful save wins over a
  too-late cancel request. No silent conversion of publication errors to success.
- Modal is keyboard accessible, restores focus, reports stages politely, and makes
  Escape/backdrop equivalent to requesting cancellation while work is cancellable.
  During the OS save dialog, tell the user to cancel that dialog to return.

## Progress

Preparing pages; checking document; copying pages; preserving document features;
applying edits; preparing saved file; writing file; finishing save. Only preparing,
copying and applying expose determinate page progress. Throttle page events without
throttling cancellation checks. No sensitive source text or credentials in events.

## Verification

Native: cancel before normalization, during copy/application/preservation, before
publication and during temporary writing; single commit boundary, late cancel,
existing destination preservation, cleanup, locked/read-only destinations, and
protected saves. Saved controlled/uncontrolled results must match independently in
page content and document features. Job owner/start/expiry/abandonment tests.

Frontend: registration race, foreign/late progress, failure/cancel retry, late commit,
dirty state for editable versus flattened output, keyboard cancellation, blocking,
unmount cleanup and rapid double starts. Full frontend/build after focused checks.
Packaged Windows: large-document cancel/retry/success and native save-dialog cancel;
inspect real output with an independent reader and preserve source/destination hashes.
One scoped final correctness review; retain evidence and runnable checkpoint. No
release or parity claim.
