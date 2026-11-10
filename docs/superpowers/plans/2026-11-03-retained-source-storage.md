# Retained Source Storage Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline, task by task.
> User authorization covers autonomous implementation; do not ask again between tasks.

**Goal:** Reduce retained-history memory while preserving all 100 Undo actions,
exact PDF bytes, permissions, source authority and missing-original recovery.

**Architecture:** Worker-owned immutable source handles hold independent block
backing and a stable private identity. Cold data is reconstructed into fallible
owned byte leases. Budgeted raw/encoded caches spill encrypted blocks through
anonymous file handles; no PDF rewriting or inter-version delta chain.

**Tech stack:** Existing Rust engine, Arc byte leases, worker-local Rc entries,
flate2 1.1.10 with its existing default miniz backend, tempfile, SHA-256, existing Windows private storage.

**Spec:** `docs/superpowers/specs/2026-11-03-retained-source-storage-design.md`

## Global constraints

- Source admission: 1 byte through 512 MiB; blocks: 1 MiB; zlib level: 1.
- Retain compression only when savings exceed 256 bytes.
- Encoded-memory payload budget: 64 MiB; raw-cache budget: 128 MiB, permitting
  the single most-recent source alone to exceed it up to existing source limits.
- Four idle native readers remains an additional cap. External leases and active
  operation buffers are outside cache-owned budgets; do not claim a process cap.
- No source/permission/annotation authority from byte-hash equality alone.
- No plaintext disk fallback, lost history, original-file overwrite or remote release.
- Preserve accepted 0.33, failed attempts and evidence. No commits or destructive cleanup.
- Serialize native compilation and desktop observations. Bind shared fixtures with
  `scripts/native-test-inputs.py` as well as product fingerprints.

## Review focus

1. Equal-byte independent captures and legacy unbound recovery must never gain
   annotation authority through cold reload (tasks 1–2).
2. External Arc owners must survive source close/trim without keeping a hidden
   weak raw allocation alive forever (tasks 1–2).
3. Disk/crypto failure while a new candidate is unpublished must preserve the
   accepted document, prior history and original bytes (tasks 1–3).
4. Multi-source export/recovery must retain all necessary byte leases across
   borrows and cancellation; metadata-only queries should remain cold (tasks 2–3).
5. Mostly incompressible content must exercise real encrypted spill and bounded
   reads; ordinary text-only memory results are insufficient (tasks 1 and 3).

## Task 1 — Independent protected source backing

**Files:** Create `src-tauri/src/source_storage.rs` and its focused tests; add the
module to `src-tauri/src/lib.rs`, direct locked flate2 dependency to Cargo.toml,
and the history-block purpose to `src-tauri/src/private_storage.rs`.

**Interfaces (crate-private):**
- `SourceStore::new() -> SourceStore` and
  `capture(&self, bytes: Arc<[u8]>) -> std::io::Result<SourceHandle>`.
- Cloneable `SourceHandle::lease(&self) -> std::io::Result<Arc<[u8]>>` and
  `identity(&self) -> &SourceIdentity`.
- Cloneable `SourceIdentity::same_source(&self, other: &Self) -> bool`,
  `len(&self) -> usize`, `sha256(&self) -> &[u8; 32]`. Identity includes a private
  fresh nonce used to bind sealed blocks. No public/wire constructors.
- `SourceStore::trim_raw_cache(&self, before_evict: impl FnMut(&SourceIdentity))`:
  invoke callback before each raw-cache owner is dropped; no I/O in trimming.
- `SourceStore::usage(&self) -> StorageUsage`, with raw cache bytes, resident
  encoded payload bytes, live source count and spilled source count for tests and
  native diagnostics. Do not expose new user-facing automation commands.

- [x] Write failing tests for raw/compressed/spilled exact reconstruction,
  non-expansion, identity distinction, cache budgets, large-most-recent exemption,
  external lease survival, weak-entry pruning and backing lifetime.
- [x] Add malformed length/codec/order/nonce/hash/trailing/truncation controls,
  wrong-purpose decrypt refusal, failed capture with a controlled unwritable spill
  destination, and successful retry without corrupt cache installation.
- [x] Run those tests and retain the actual failing output.
- [x] Implement immutable descriptors, bounded codec, current-user sealed block
  backing and cache policy. Hold anonymous file handles; admit backing before
  registering the new source. Use test-only small limits/fault injection.
- [x] Run the focused tests and private-storage regressions; require zero failures.
  A small isolated test crate may compile the actual module for fast iteration,
  but the Folio library must also compile and pass these tests before integration.

## Task 2 — Engine leases and source identity

**Files:** `engine.rs`, `native_residency.rs`, `imported_overlay_sources.rs`,
publication/transfer helpers, all existing source-byte consumers found by `rg`.

**Interfaces:** `OpenDocument::bytes() -> EngineResult<Arc<[u8]>>`; store a
`SourceHandle` in `source_bytes`. `ResidentDocument` takes the same handle and
leases bytes while loading; add an explicit native eviction method. Change
`Index::capturing/unbound` to consume private identity, and `bound_to` to compare
the corresponding identity. Public engine byte-returning APIs stay unchanged.

- [x] Add failing worker regressions for forced cold source loads, native eviction
  before raw eviction, equal-byte foreign identity rejection, trusted annotation
  replay and legacy-unbound refusal after repeated materialization.
- [x] Add the worker store and migrate every constructor. Capture before inserting
  candidates; failed operations must close unpublished assets and source handles.
- [x] Migrate consumers explicitly to fallible leases. Hold named Arc owners for
  borrowed multi-source forms/composition; audit persistent paragraph contexts,
  read-page caches and native wrappers for unintended full-source owners.
- [x] Integrate request-end raw/native trimming, preserving the current reply
  cleanup rules and metadata-only behavior. No automatic raw Deref or empty-byte
  placeholder is allowed.
- [x] Verify recovery policy, source publication and imported annotation snapshots
  against the stable identity. Do not remove the current semantic checks.
- [x] Run focused worker, imported-overlay, recovery, source-protection, text,
  form/image, complete-source save and cancellation suites. Then run the entire
  native suite with source/shared-input capture; inspect terminal target totals.

## Task 3 — Sustained native and desktop acceptance

**Files:** New evidence under `artifacts/retained-source-storage-20261103`, existing
native workload examples and fresh owned desktop harnesses copied with verified
input identities. Preserve inherited evidence rather than relabeling old results.

- [x] Repeat the 101-version small/large native histories with forced cold leases
  and independently audited saved pages; compare exact bytes with accepted 0.33
  source-equivalent outputs where applicable. Record storage-owned cache bytes,
  spill usage, external leases and sampled process memory separately.
- [x] Add an incompressible generated multi-version case, storage creation/read
  failures, and corrupted/misbound protected blocks. Require preservation of every
  accepted state and exact retry results. Direct sealing/write/flush fault injection
  remains an explicitly recorded coverage gap.
- [x] Build a fresh optimized local GUI, then run the small/large 105-edit sessions,
  oldest retained Undo, full Redo, discarded Redo, close, saved reopen and
  missing-original recovery with unrelated documents forcing eviction.
- [x] Audit all saved pages independently. Attribute memory changes only across
  matched display/environment runs; retain actual failure records.
- [x] Request one final implementation review, correct consequential findings
  ourselves with reproducing checks, and record source/binary/evidence identities
  and remaining limits in the quality ledger. Do not package or claim parity yet.

## Execution record

Design-builder check complete: no consequential questions. Task 1 has 13 focused
passes in the isolated actual-module harness and 1,488 library / 5 private-storage /
23 recovery passes in the Folio correctness gate (10 library skips). Product and
shared-fixture fingerprints match before/after. Engine integration remains next.
Ruling: preserve the existing miniz backend, because enabling zlib-rs directly
would also change PDF compression through Cargo feature unification. The first
native compilation was deliberately canceled with exact owned-process evidence;
its result is not accepted. Corrected backend measurements and checks are required.


Engine integration update: integration-library-1 passed 1,495 tests with zero
failures and 10 existing ignored tests. Five forced-cold worker regressions are
included. Failed attempts and their corrections are retained under
artifacts/retained-source-storage-20261103. The optimized integration/binary/doc
run is active under integration-native-1, bound to the same source fingerprint
c71f01a35f7784094890c081366dadf951dbaea324f43483cf344b7b36aa9514.
No sustained-memory, desktop or release acceptance is claimed yet.

Task 2 complete: integration-summary.json binds 1,495 library and 1,154 integration passes, 31 existing skips, 133 integration targets and successful binary/doc harnesses. Product and shared inputs unchanged. Task 3 remains pending.

Task 3 complete — 2026-11-09: checkpoint.json binds corrected native, desktop, recovery, review correction and internally matched per-case resource observations. New portable qualification is a separate plan; no parity claim.
