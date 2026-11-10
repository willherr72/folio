# History cache headroom implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline; one fresh final review. Standing user autonomy replaces routine design/permission questions.

**Goal:** Pass the original full-history memory/fidelity targets with useful headroom, retaining every100Undo/Redo action and exact source bytes.
**Architecture:** Test64MiB default decoded cache via existing eviction/cold source backing. Encoded64MiB/identity/spill/security semantics stay unchanged. Failure remains failure if this hypothesis is insufficient.
**Tech stack:** Rust SourceStore/private storage, existing native/Windows/independent-reader harnesses.
**Spec:** docs/superpowers/specs/2026-11-10-history-cache-headroom-design.md

## Global constraints and review focus

Keep036launcher and all failed037 artifacts/originals. No forcedGC, viewport/style changes, history reduction or target relaxation. Serialize compilers/tests/UI/resource work; one Cargo job and existingD:buildcache/temp. Product change requires fresh native/binary evidence, not037metadata-only reuse. Candidate includes complete037runtime resources. Review external raw leases, eviction-before-native-close order, cold reload identity/bytes, spill/error retry behavior, small/large latency tradeoffs and actual qualified resource scope. No public release/commit/push or auto userapp launch.

### Task1: behavior regression and policy

Files: src-tauri/src/source_storage.rs, source_storage_tests.rs; artifacts/history-cache-headroom-20261110 baseline/test receipts.

- [ ] Verify actual failed resource sample/process breakdown and preserve full current source83db plus package identities. Keepfailed037unqualified and remove only identity-checked test profiles after exit.
- [ ] Add default-policy regression using real independent multi-MiB captures/trim: raw cache<=64MiB, every source handle persists, full exact cold reload, usage clears on drop. Watch functionalRED under128MiB policy.
- [ ] Change only decoded default128→64MiB; run GREEN plus real storage/private payload/corruption/external lease suite. No data/history/encoded limit change. Record source and exact commands/dependency hashes.

### Task2: current native and actual GUI qualification

Files: new source/native/build/diagnostic/desktop receipt helpers; copy immutable300-page source/golden and complete037resources with provenance.

- [ ] Fresh affected/full native correctness under original profiles, source/input guards, normal optimized GUI with profiling disabled. No unqualified artifact reuse across product change.
- [ ] Actual same300-page105edits/100Undo/100Redo/save/reopen; independent all300pages/8controls, original512MiB/1GiB sample targets, identity-bound cleanup. Record source-owned counters where available and all process roles; no peak/leak attribution claim.
- [ ] Check selected small/quiz/shared-form/font, cold recovery/spill/error cases and actual latency tradeoff. Preserve failures; if64MiB does not provide real useful headroom, diagnose/revise without weakening gates.

### Task3: review and integration authority

- [ ] Freeze actual source/helpers/binary/receipts and public-only review context. One fresh final review; grade actual effect, inline consequential fixes with RED/GREEN and affected rechecks; record minors/rulings/limits.
- [ ] Historical seal qualified implementation and leave broader goal active. New versioned combined desktop/CLI portable needs its own integration;036launcher stays until that package qualifies.037failedcandidate never promoted.

Self-review: storage behavior test owns policy; fresh native and complete-resource GUI own actual user/history/performance/memory claims; final review/seal follows real evidence. Baseline ownership observations are hypothesis support only; no product change or successful fix is claimed yet.
