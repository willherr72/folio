# History-bound text removal workflow

## Intent and chosen approach

Deliver the visible workflow supported by the verified native kernel: prepare the
actual edited document, mark text, inspect a before/after preview, Apply once,
Undo/Redo, then save an editable copy through existing atomic Save. User requests
autonomous work; implementation choices below are ours, with no routine questions.

Approaches: drawing on the main canvas would compete with its many editing modes;
using the original PDF bypasses unsaved edits. Choose a dedicated modal with the
owned composed snapshot and before/after page views. Existing Modal supplies focus
containment and blocks background editing. This is text removal, not a secure area
redaction product: the current strict engine refuses unsupported PDFs intact.

## Document transaction

Capture exact tab ID, adapter and history object on entry; deep-copy/freeze the
document plan for native requests. A request is current only when the same tab is
active, its adapter and history object are unchanged, and the app is mounted.
Validate before native Apply and again before adoption. The modal normally blocks
tab/history changes, but asynchronous correctness must not depend on that alone.

All returned source IDs must be new relative to every borrowed source in every
tab, and prepared/candidate IDs must be distinct. Retain ALL original source IDs
in the tab after Apply, even those absent from its current pages. One history
commit replaces all pages with the applied complete source, preserving occurrence
page IDs and selected page ID, using sequential page indices, returned physical
dimensions, rotation0 and empty overlays/no crop/contentSourceId. Use bookmarks
read from the candidate, remapped to preserved page IDs; do not resurrect removed
bookmarks or unreadable-bookmark state. Keep document name and savedDigest, clear
selected overlay. Undo restores the exact previous document; Redo restores the
same result without another native edit. Sources live until tab close.

Prepared page count/dimensions must match composed display geometry (crop and
workspace rotation included; tolerance0.01pt). Candidate must match prepared
geometry. Reject returned protection, bookmarkError, nonempty overlay DTOs,
invalid dimensions/IDs and malformed preview counts/page indices. Native Apply
must return the exact preview candidate document. No guessed source substitution.
Reject entry when bookmarkError exists: existing explicit discard workflow owns it.

## Async ownership

Introduce a plain TypeScript workflow controller around the native session API,
separate from React. One operation at a time; no automatic retries or queued edits.
It owns the public native session and uses session release for prepared/pending
copies. Cancel/dispose is immediate and idempotent. A late Begin must be released;
late Prepare/Preview must not update UI or commit. Native release handles their
ownership. A late/adoption-refused Apply closes only the known, validated candidate
through its creating adapter, never a response-supplied unknown/borrowed source.
Apply failure invalidates that preview; a new Preview is needed before another
Apply. Successful adoption transfers candidate ownership to the tab before release.
Cleanup failures remain host-owned until explicit close/window destruction; do not
claim immediate reclamation when IPC fails. React StrictMode creates a fresh
controller per effect generation and disposes the old one.

Controller callbacks query current workspace ownership/currentness at use time.
Deep-copy each settled DTO before storing; adapter mutation must not alter owned
preview data or submitted page plans. Marks are copied and validated against the
prepared physical page dimensions,1..64rectangles,finite positive in-page bounds.
Any mark change invalidates the old preview and acknowledgement; discard pending
preview via its session, not generic close. Bounds requests are paginated≤1000 and
validate page/offset/total and finite in-page boxes against preview summaries.

## Visible interaction

Expose native-only "Remove text…" beside text/OCR tools when the full capability
set exists. Disable for protected/unauthorized input pages, empty/over1000pages,
busy/modal states, or unreadable bookmarks (explain the latter in the dialog if
entry is invoked programmatically). No demo capability or automatic publication.

Dialog starts by preparing the current plan. Page navigation offers previous/next
and a numeric page control, bounded to actual prepared pages. Source raster is the
immutable prepared source; candidate raster is the exact preview source. Render
through renderPage with latest-only response handling and blob URL disposal.

Draw a rectangle on source with pointer capture, clamp within page bounds, cancel
on pointer cancellation; numeric Left/Top/Width/Height fields plus Add selection
provide keyboard parity. Up to64marks across pages, removable from a labeled list,
plus Clear selections. No synthetic whiteout overlay. Native refusal leaves the
source/marks intact, shows error and keeps Apply disabled.

Explicit Preview calls native once. Show affected character/page counts, page
numbers and all omission categories. Native glyph boxes paint a bounded-resolution
canvas over the source, fetched in chunks≤1000; no million-element SVG or output-
derived selection mask. Validate every chunk; clear on generation change. Fetch
only current page; no unbounded geometry cache. Candidate raster alongside it is
the saved appearance. Page navigation remains available for reviewing other pages.
Apply needs current preview, no running request, successful current-page raster
and glyph-mask completion (unaffected pages need no bounds), unexpired lease, and
explicit checkbox acknowledgement of nonempty omission list. New Preview resets
acknowledgement. Preview timeout requires another Preview; native remains authority. Add explicit
Refresh working copy: dispose old session, begin/prepare again from the same
captured plan, clear preview/acknowledgement, and retain marks only after refreshed
geometry validates against that plan. Refresh is available after preparation or
preview failure and when idle, but cannot bypass stale tab/history. This recovers
from prepared-source expiry (which another Preview alone cannot repair).

Explain once: "Removes selected text from the PDF. Other content is not redacted.
Unsupported PDFs are left unchanged. You can Undo before saving a copy." Cancel
and Escape remain available during pending operations; no modal trapping while
native work finishes. Parent callback adopts synchronously after currentness check.
After Apply close dialog, show "Text removed. Use Undo to restore it, or save a
copy." Existing Save a copy uses atomic native publication and overwrite guards.

## Verification and boundaries

TDD model/controller tests: stale tab/history/adapter, new IDs versus borrowed
sources, malformed dimensions/preview/Apply, exact page occurrence mapping,
Undo/Redo/bookmarks/digest, settled DTO mutation, lifecycle cancellation at every
await, lost/rejected Apply cleanup, no double Apply, metadata acknowledgement
owned by UI, paginated bounds validation. Dialog tests cover keyboard/pointer
marks, redraw invalidation, errors, expiry, Cancel, page navigation, bounded masks,
focus and asynchronous cleanup. App tests cover current plans, duplicate-only
selection, one undo step, originals retained, save-copy payload and second-tab
isolation. Run full frontend/build, native affected tests if native code changes.

Build a fingerprinted local desktop candidate and use the self-contained simple
PDF corpus for real Preview/Cancel/Apply/Undo/Redo/save/reopen acceptance; independent
readers compare saved text/retained positions/raster and originals. No original
user PDF writes. Preserve all failures. One final independent review; fix material
findings via RED/GREEN without a second review. Keep inherited work and artifacts,
no commit/release/app restart. Native kernel compatibility, secure area removal,
peak-memory certification and commercial parity remain open beyond this workflow.
