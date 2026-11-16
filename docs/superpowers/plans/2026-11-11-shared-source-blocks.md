# Shared retained-source blocks implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline. Standing autonomy supplies routine decisions; no per-task agents, one final fresh scoped review after actual acceptance.

**Goal:** Reduce duplicated history backing while preserving every PDF version and useful editing behavior.

**Architecture:** Store64KiB immutable encoded payloads with shared Arc ownership and unique allocation budget accounting. Keep request-local matching, per-source identity/crypto and existing two-idle-reader experimental foundation. Review the combined four-file delta against6250.

**Tech stack:** Rust, existing SourceStore/PDFium/Windows validation.

**Spec:** docs/superpowers/specs/2026-11-11-shared-source-blocks-design.md

## Global constraints

512MiB source admission, encoded64MiB/decoded64MiB budgets, independent identities/nonces/spill authority and100Undo/Redo remain unchanged. No renderer, forcedGC, DPR/style/browser intervention, threshold relaxation, commits/push/publicrelease or launcher promotion. Serialize tests/compilers/UI with oneCargojob and D:cache/temp. Preserve failed040/idle-reader qualifications and039launcher. Folioopt0 correctness does not prove optimized performance. Native evidence must be fresh after backend changes; frontend reuse only under exact unchanged hashes.

## Review focus

- Shared payload accounting across live/closed owners, per-source duplicate blocks and spill transition must neither exceed the real unique budget nor retain owners forever.
- Content equality must not merge source identities, nonces, permissions, replay or encrypted spill-frame authority; altered codec/length/hash/payload must remain guarded.
-64KiB granularity must not shrink512MiB admission or64MiB budgets, break final partial blocks, or make malformed decoding unbounded.
- Failure while spilling a source sharing earlier blocks must leave existing owners usable and permit retry without installing partial state.
- Actual native ownership counters, exact saved glyph/RGB and first natural-display memory evidence must bind to the changed combined core; estimates and sequential timings cannot certify GPU ownership, peaks or general performance.

### Task 1: shared storage RED/GREEN

**Files:** `src-tauri/src/source_storage.rs`, `src-tauri/src/source_storage_tests.rs`; `artifacts/shared-source-blocks-20261111/{baseline/,focused-red-1/,focused-green-1/,source/}`.

**Interfaces:** `SourceStore::capture(Arc<[u8]>) -> io::Result<SourceHandle>`, `usage()` counts unique resident encoded allocations; source handles/lease/identity/spill API unchanged.

- [ ] Preserve345bsource/native/frontend maps and four source bodies,6250combined baseline, actual failed firstsample/native ownership/estimate receipts. Expected: current source exactly345b; no active old controller.
- [ ] Add `varied_sources_share_unchanged_blocks_without_spilling_or_merging_identity`:16sources with64KiB varied first/last blocks and512KiB shared incompressible middle,4MiB encoded limit, no raw cache; expect no spill,<=3MiB encoded,16distinct identities and exact all cold reconstructions. Add identical-content ownership/accounting/last-owner tests. Run before production edits. Expected: requirements fail under current unshared storage, not compiler errors.
- [ ] Implement64KiB chunking with independent byte constants for512MiB and64MiB limits; Arc payloads, request-local matching by codec/length/raw hash plus exact encoded bytes, unique allocation accounting. Keep encrypted spills per-source and no global strong pool. Update mechanical test payload constructors only as required. Existing budget/failure tests must use distinct bytes when testing pressure rather than accidental duplicate captures.
- [ ] Run source-storage, retained-source, residency and security/spill tests. Expected: all pass including last-owner release, external leases, malformed/trailing/truncated decoding, source/ordinal/codec authentication and retry. Complete task using read-only observed RED/GREEN and exact combined source-delta contract.

### Task 2: actual ownership and fresh GUI qualification

**Files:** New root `artifacts/shared-source-blocks-20261111/` native/ownership/GUI/desktop/handoff evidence and pinned helper provenance.

**Interfaces:** Task1 fingerprints bind fresh ordinary native correctness and optimized GUI. Exact unchanged frontend evidence1517/226 may be reused;25runtime resources come from preserved040package with exact hashes.

- [ ] Run fresh full native gate: optimized integration/bin/doc and complete Folioopt0 library correctness, source/input pre/post checks. Expected: zero failures/unfinished targets; actual counts/skips recorded.
- [ ] Run actual generated worker ownership workload against changed source, retaining101distinct versions, exact cold reload and close/reopen; include varied-content storage regression. Expected: actual shared encoded ownership materially decreases without identities/source loss; logical counters zero after final close, no performance claim from opt0.
- [ ] Qualify unchanged frontend and build fresh normal optimized GUI/resources; use correct directory staging. Expected: exact source/native/executable/resource identity and fresh TS/Vite build.
- [ ] Run first unchanged current-naturalDPR1.5dense300page105edit/100Undo/100Redo, independent all300glyph/RGB/eightcontrols, owned root/profile cleanup and original512MiB native/1GiB tree targets. Expected: all pass; failure is retained and investigated, no unchanged rerun or raised limit.
- [ ] Verify actual small/quiz/form/font cold ownership/handoff and selected latency summary. Expected: three actual independent source-based checks and exact cleanup, selected median ratio<=1.5or explicit diagnosis before acceptance. No broad peak/leak/printing claims.

### Task 3: final combined-component review and historical closure

**Files:** Frozen public four-file review packet/report, dispositions, final source/helper/output/resource index and component checkpoint.

**Interfaces:** Successful Task2 source-bound evidence feeds one final review; later combined portable integration remains separate.

- [ ] Request one fresh scoped review of combined6250tofinal delta, exact Review focus and ledger rulings, excluding private quiz raw data. Fix consequential findings inline RED/GREEN and affected suite; record declined judgments/minors/costs.
- [ ] Seal only after real acceptance/review. Expected: historically verifiable checkpoint and honest limits; all old failures,039launcher and worktree/ledgers preserved, broad goal active, no package promotion.
