# Retained PDF source storage

## Intent and scope

Reduce long-session memory without shortening Folio's 100-action Undo history,
changing saved PDF bytes, or weakening source-bound permissions and annotation
ownership. Keep all existing desktop/CLI/MCP workflows and missing-original
recovery. The user authorized autonomous implementation; the choices below are
coordinator decisions, not claims of separately approved design details.

0.33 is the preserved accepted package. The previous turn made progress by
completing its extracted acceptance and checkpoint. This work is a new milestone.

## Evidence and approaches

`artifacts/retained-source-storage-20261103` contains read-only optimized Rust
compression probes of 202 previously audited history versions and four fixtures.
Both whole-source and 1 MiB block experiments reconstruct every byte. A scanned
fixture barely compresses; compression alone cannot bound retained memory.
The earlier prefix/suffix delta experiment retained almost all original bytes.

1. Cross-version chunks/deltas can share identical content, but add indexing,
   collision handling and retention dependencies. The current evidence does not
   justify that complexity before a bounded independent storage path exists.
2. Independently compressed snapshots are simpler to verify and randomly access,
   but incompressible sources still need storage outside the resident cache.
3. **Chosen:** independent block-compressed snapshots, bounded encoded-memory
   storage, and encrypted anonymous-file spill. This preserves independent Undo
   states and uses existing Windows private-storage protection. No PDF parser or
   serializer participates in storage; the original byte sequence is authoritative.

Use 1 MiB blocks and zlib level 1 through the already locked flate2 1.1.10
default miniz backend. The initial probe used zlib-rs, but Cargo feature
unification would switch existing PDF serialization to that backend. Preserve
the existing backend; separate corrected measurements are required. Keep compressed data only when it saves more than 256 bytes;
otherwise store the exact raw block. Level 6 saves more but costs more encoding
time in the probe. Revisit only if actual editing acceptance warrants it.

## Store and lifetimes

Create a private worker-owned `SourceStore` and immutable `SourceHandle` in
`src-tauri/src/source_storage.rs`. The store registry contains weak **entry**
references; a handle owns an entry. Do not retain `Weak<[u8]>` references: they
would keep the raw allocation reserved after its last strong owner disappears.

Each capture creates a fresh `SourceIdentity`, containing length and SHA-256,
with allocation identity independent of the raw buffer allocation. Equal bytes
captured twice have distinct identities. A cloned handle retains its identity.
Identity construction and storage mutation are private; identity is not a wire
format or a permission claim. Existing source IDs and policies remain unchanged.

`SourceHandle::lease()` returns a fallible owned `Arc<[u8]>`. Cache hits clone the
same Arc. A cold lease reconstructs into a private buffer, validates block order,
lengths and complete source SHA-256, and only then installs the cache entry.
Incomplete/corrupt decoding never installs partial bytes or a permanent error.
External leases remain valid across eviction and source close. The public
`PdfEngine::source_bytes()` signature stays unchanged.

Store data is worker-local (Rc/RefCell is acceptable); raw Arc leases cross worker
boundaries. No reference into mutable cache state may escape an API call. Use
fallible access, not an infallible Deref that could panic on cold data.

## Storage and cache policy

- Existing source admission remains 1 byte through 512 MiB.
- Encoded block payloads retained by live entries have a **64 MiB** aggregate
  memory budget. Capture under that budget in memory. If a new source cannot fit,
  spill that source into an anonymous file, including any blocks already encoded.
  Metadata is bounded by at most 512 block descriptors per source; no delta chain.
- Spill stores only ciphertext. Add a separate private-storage purpose for
  history source blocks. Each sealed block binds its source identity nonce,
  ordinal, original length, codec and encoded payload; keep expected block hashes
  and lengths in the immutable in-memory descriptor. Bound every stored read and
  decoded allocation before processing. Reject reordered, duplicated, truncated,
  extended, wrong-purpose and corrupt blocks.
- Retain an owned anonymous `tempfile` handle, not a reusable pathname. Windows
  closes/deletes it when the last source owner/process goes away. Do not create
  plaintext files or reuse the recovery folder or input path. Use the existing
  current-user protection with no plaintext/non-Windows fallback.
- Complete backing creation before publishing a new source. Failure leaves the
  original document/history/caches usable and returns an actionable operation
  error; never silently shorten history or accept a source without its backing.
- After each complete worker request, aim for **128 MiB** of cache-owned raw
  buffers by LRU eviction. Permit the single most-recent source alone to exceed
  that budget up to the existing source limit, to avoid reopening a large current
  document on every operation. Evict its native reader before releasing a victim's
  cached raw Arc. Keep the existing four-idle-reader maximum as an additional cap.
- External leases, active request buffers, native allocations and WebView memory
  are outside these cache-owned budgets. Report that distinction. This does not
  claim a process peak-memory limit or guaranteed operation on a full disk.

## Integration and authority

Replace `OpenDocument.source_bytes` with a `SourceHandle`; provide
`OpenDocument::bytes() -> EngineResult<Arc<[u8]>>`. Migrate consumers explicitly,
including long-lived multi-source borrows; keep leases alive for each operation.
Capture new handles at every constructor before insertion/reply. Do not infer
immutability from byte hashes supplied by a caller.

`ResidentDocument` retains a handle and acquires a byte lease when loading PDFium.
Metadata-only requests should not materialize bytes. Request-end trimming must
release matching native handles before dropping raw caches; no native page/font
reference may survive its request.

`imported_overlay_sources::Index` retains the private source identity, not raw
bytes. `bound_to` compares that identity by allocation and still requires trusted
state. Snapshot hashes derive from the same immutable captured source. A cold
reload retains authority because reconstruction is verified and the identity
never changes. An equal-byte foreign capture, forged recovery metadata, and a
legacy unbound index must still fail to acquire authority. Reuse the established
semantic/popup/alias verification; hash equality alone never authorizes an edit.

Recovery receives an ordinary owned byte lease, then preserves the existing
source-policy and annotation snapshot checks and encrypted durable publication.
Anonymous history spill is not crash recovery. Asset ownership, form-admission
markers, source close, abandoned Redo cleanup, save destinations and protection
restrictions remain source-version scoped.

## Verification and completion

First prove the store with small injectable budgets: exact raw/compressed/spilled
round trips, no retained expansion, independent identity, leases surviving trim
and close, released entry/file ownership, bounded malformed decoding and read
lengths, wrong-purpose/nonce/order rejection, failed capture and retry. Preserve
actual red/green evidence. Tests may inspect private state; product commands may
not expose ciphertext, private paths or source capabilities.

Then exercise actual worker eviction/reload: annotations and popup removals,
legacy unbound recovery, protected sources, fields, images/fonts, complete-source
publication, cancellation and dropped reply cleanup. Audit all persistent raw-byte
owners; compiler fixes alone are insufficient. Re-run affected native suites, then
the full native suite with the shared-fixture capture helper. Frontend tests can be
qualified unchanged unless integration changes their inputs or behavior.

Finally repeat the preserved 101-version native and 105-edit desktop histories,
including both 2-page and 300-page documents, forced cold reads, oldest Undo,
Redo invalidation, close, missing-original recovery and all-page independent
saved-output audits. Add a mostly incompressible history to prove spill, and fault
injection to prove failed disk/crypto work never loses an accepted state. Match
display/environment before attributing desktop memory changes. One final scoped
implementation review; coordinator verifies small corrections. Preserve 0.33 and
all failed attempts. Packaging is a later milestone after implementation acceptance.
