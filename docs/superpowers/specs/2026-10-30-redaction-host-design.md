# Desktop ownership for text-removal previews

## Intent and boundary

Connect the verified private text-removal engine to the desktop transport. This is
the ownership prerequisite for visible marking, preview, Apply and save-copy;
it does not by itself complete that workflow or establish commercial parity.
The user authorized autonomous execution. Decisions below are the implementer's.

## Approaches

1. Pass native owner IDs to the renderer: simple, but lets caller data authorize
   another session and leaves cleanup dependent on the webview.
2. Retain owners in a trusted window registry (chosen): public opaque session
   handles are checked against the invoking Tauri window, with explicit dialog
   release and native destruction cleanup. Reuses the existing native worker.
3. Compose every editor operation inside a new native document model: would
   duplicate existing editor history and composition; unnecessary for ownership.

## Contract

- New redaction_host.rs owns PdfEngine and window entries. Setup registers actual
  windows. Commands never lazily recreate a destroyed window. Each entry is an
  Arc with an atomic alive flag and mutex-protected sessions/adopted IDs. A queued
  command captures its entry before spawning blocking work. Destroy removes that
  entry and clears alive immediately, then cleans asynchronously; old work cannot
  attach to a new window with the same label.
- begin returns an independent UUID session handle. Native owner UUID never
  leaves the host. At most 16 sessions per window (native global 16 still applies).
  Preview, bounds, Apply, discard and release all require the same live window
  and registered session. Wrong-window/session attempts cannot consume a lease.
- Serialize a window's operations through its mutex. Check alive before and after
  native calls. Destruction during a call makes its result unusable and releases
  its new resources before returning an error. Destruction cleanup is idempotent.
- Pending previews remain native session-owned (existing 4-preview/128MiB/32MiB/
  15-minute limits). Release closes pending previews; Apply consumes a lease once.
  Applied documents become window-owned and survive dialog release for history.
  Track at most 4096 adopted document IDs per window; check capacity before Apply.
  The ordinary close_document command removes a successfully closed adopted ID.
  Destroy closes all remaining adopted results, including a successful Apply
  whose IPC reply was lost. Closing a dialog does not delete adopted history.
  Source documents are never adopted or closed by this registry.
- IPC handlers use WebviewWindow identity, not a request owner string. All engine
  calls and cleanup run off the UI thread. Request/response types use camelCase.
  Only input mark/rectangle types deserialize; unknown mark fields are rejected.
  Native validation remains authoritative (1..64 finite marks, bounded pages,
  current source/rights, bounds page size 1..1000). No disk writes in these calls.
- Commands are explicitly named source operations. They accept one immutable
  source ID, not a PagePlan pretending to represent the current edited document.
  The public FolioAdapter gains optional typed methods, with transport tests.
  No button is exposed until the composition and UI acceptance below are met.

## Full editor workflow integration requirements (not satisfied by this bridge)

The visible tool must preview the current edited document, not its original file.
Snapshot current history/adapter/tab identity. Compose the ordered current page
occurrences and overlays into an owned source with edit/copy authorization, then
inspect that exact source. Duplicate pages get independent output indices. Crop
and rotation coordinates must be projected consistently; bookmarks, forms,
annotations, images and unsupported fonts must be preserved or explicitly refused
without publication. Never silently omit these to gain admission to the current
text-only kernel. Metadata removal must be disclosed before Apply. This tool
removes selected text glyphs, not all content in an area; do not label it secure
area redaction. Apply validates the history snapshot and adopts a complete result
atomically; Cancel/closed tab drops unadopted work. Save-copy must use existing
atomic publication and source overwrite guards. Undo retains the original in
memory, so saving a sanitized copy and undo history are distinct concepts.

## Verification

Real worker tests prove cross-window/session denial, release cleanup, one-shot
Apply, adopted survival across release, destruction cleanup, original survival,
close bookkeeping and bounded sessions. Deterministic barriers exercise queued
work and destruction during work, plus old/new window generations. DTO and
frontend transport tests pin exact envelopes and absence of caller owner tokens.
Run focused native tests, library regression, TypeScript build and relevant
frontend tests. Freeze source hashes for one final independent review and fix
material findings with reproduced regressions. No package or desktop acceptance
claim until an executable is built and the full workflow is exercised.

## Limits

This adds ownership, not a sandbox for a compromised renderer: older general
document APIs remain app-wide. Applied results with lost replies live until
window destruction or explicit document close; they are bounded in count, not a
new whole-process byte quota. The parser/native PDFium limitations from the loader
checkpoint remain. No external dependency, network, original PDF mutation,
inherited-work cleanup, or release is needed.
