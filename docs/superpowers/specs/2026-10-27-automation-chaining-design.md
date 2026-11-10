# Consecutive edits in one MCP session

The prior milestone is verified progress: inspection, preview and exact Save work.
Next, let a caller keep editing an accepted preview without restarting MCP or
authorizing arbitrary file paths. User delegates design and execution without
routine approval loops. Preserve native fidelity proofs and all prior artifacts.

## Approach and choices

Use immutable document versions. Add `folio_apply_preview {previewId}` to copy the
validated candidate into private session storage and return a new `documentId`.
Callers may inspect/edit/render/export it immediately. Existing source IDs and
previews remain valid. Save is still explicit and publishes only the requested
preview; applying does not write a public export. Applying before or after Save
uses identical candidate bytes, so a saved edit can be continued without restart.

Automatic registration on every Save would impose new capacity failures on callers
who only want an export. Reopening arbitrary paths broadens authorization. Neither
is needed. Copying the candidate into private storage instead of hardlinking the
public saved file prevents later public-file modifications from changing a session
source. Do not introduce a persistent native engine.

Add `folio_close_document {documentId}` to release an unused immutable input,
including launch inputs. It does not remove originals, exports or already-created
previews. Unknown/closed IDs fail; never reuse an ID even after closure. IDs stay
`document-N`, using a checked monotonic counter. Protocol resolves actual live IDs,
not vector positions or counts. Listing shows live documents and retained previews.

All live document versions, including launch snapshots, total at most64 documents
and512MiB. New copies are charged in full even for identical bytes. Capacity refusal
precedes the worker and leaves previews/exports intact. No document TTL or silent
eviction: explicit close or session end releases them. Preview8/512MiB/15min limits
remain separate and unchanged. Existing worker temporary-file limits still apply.

## Publication and ownership

Apply runs existing CLI save-text-preview in the usual disposable supervised worker,
targeting fixed `Edited.pdf` inside its private operation directory. This verifies
manifest, both PNGs and candidate bytes, syncs and publishes exclusively. No new CLI
command or model-supplied path. Store the expected candidate byte count/hash in the
owned Preview record so parent capacity/adoption can validate the bounded reply.

After successful worker exit, adopt only its fixed regular nonsymlink file with the
expected size and child-reported SHA256. Supervisor does not hash512MiB; the child
already verified/copy-hashed it. On Windows open the retained input with delete-on-
close as for launch snapshots. Own the file handle before its private TempDir;
drop handle before directory. Recheck count/byte capacity on adoption, compose
reply before publishing the new ID. No candidate/private snapshot path in Apply
reply, only documentId/name/bytes/sha256. Failed adoption/cancel drops the owned stage.

Close, Apply, Save, Discard and all native operations return busy while a worker
runs. Discovery/list/ping/cancel remain responsive. No source removal or preview
expiry while consumed by an active worker. Drop/reap worker before previews and
documents at EOF/output failure. A lost Apply reply is recoverable via listing.
Hard process crashes can leave an empty operation directory; no OS isolation against
hostile same-user mutation or general crash-recovery guarantee is claimed.

## Testable supervisor

Extract current call/cancel/poll/prune ownership into mcp/session.rs, used by the
stdio loop. This small state holder owns active worker first, previews second,
config/documents last so field destruction is safe. It permits deterministic unit
tests with an owned sleeping process and aged preview records, without shipping
environment hooks, sleeps in product code or changing native worker deadlines.
Tests cover active preview/save/apply cancellation, busy close/discard, expired
preview pinned during an active worker and cleanup after worker release.

## Acceptance

First prove missing Apply/Close with actual MCP subprocess tests. Cover two edits
to one document in the same session, independent original/derived IDs, exact
candidate hashes, public export unchanged until explicit Save, preview discard
independence, closed IDs not reused, last document close, malformed paths/IDs,
capacity refusal/recovery and EOF cleanup. Test count and aggregate limits without
allocating huge PDFs using deterministic config fixtures. Add a real CLI regression
that selects the second of identical visible runs and preserves the first.

Run library/CLI/MCP/inspection/automation regressions, then a fresh portable build
and official SDK legacy/modern multi-edit workflows on generated public Word PDFs
outside the checkout. Independent readers verify final text, surrounding/other-page
pixels and a real bookmark/note. Preserve earlier successful and failed artifacts.
One fresh scoped final review, inline implementation, no commits/push/releases.
Overall Adobe/Foxit-quality goal stays active; paragraph and nested automation,
client-host UI acceptance and broader format coverage remain work ahead.
