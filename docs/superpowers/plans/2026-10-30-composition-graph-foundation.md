# Composition graph foundation implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Supply a transactional, bounded object relocation primitive for faithful structured composition without enabling an incomplete Save route.

**Architecture:** Move already parsed source objects into a private accumulator, assigning every source object a distinct output identity before rewriting references. Preserve physical source provenance separately from future logical editing lineage. This primitive does not parse, merge catalogs, project pages, finalize annotations, or publish.

**Tech Stack:** Rust, existing local lopdf, existing offline Cargo test infrastructure.

**Spec:** `docs/superpowers/specs/2026-10-30-structured-composition-design.md`, including builder-check dispositions.

## Global constraints

- Keep the existing structured Save refusal and byte-exact complete-source route unchanged.
- Preserve all prior dirty work, original inputs, candidates and failed evidence. No commit, push, release or app restart.
- One native workload at a time; locked offline Cargo, one build job, existing shared target.
- The primitive bounds accumulated stored payload, object/value counts and direct nesting. It does not promise bounded parser allocation, decompression, total RSS or interruptible parsing; those require separate admission work before route integration.
- No stream decompression, content regeneration, pruning or reference resolution by geometry. Indirect cycles and aliases must survive. Dangling references, nonfinite numbers and invalid object zero are explicitly refused for this new primitive.
- Logical failure/cancellation leaves the accumulator unchanged. Consumed input documents need not be returned on failure; immutable source bytes remain caller-owned.

## Review focus

- Same object numbers with different generations or from different sources must never collide.
- Every object, including a currently unreachable one and trailer values, participates in validation and rewriting.
- Limits must accumulate across imports, including dictionary keys and stored stream bytes; they are not claims about decompressed/rendered size.
- Cancellation must remain transactional after reference rewriting has begun.
- Source offsets in moved streams must not survive as output offsets; encoded content and compression policy must survive without a second payload copy.

## Task 1: Transactional graph relocation

**Files:** Create `src-tauri/src/composition_graph.rs`, `src-tauri/src/composition_graph_tests.rs`; register internal module in `src-tauri/src/lib.rs`.

**Interfaces:** `CompositionGraph::new()`; `import(source_key: usize, source: lopdf::Document, check: &mut impl FnMut() -> EngineResult<()>) -> EngineResult<()>`. Private document, source maps, usage and limits remain encapsulated until graph-native finalizers are built. Each imported record keeps original PDF version, remapped trailer and a complete source-ID/output-ID map. No public API.

- [x] Add executable red tests for disjoint source identities, generations, forward/cyclic/alias references in dictionaries/arrays/streams/trailer, move-only stream storage, invalid references/nonfinite values, duplicate source registration, cumulative limits and cancellation rollback. Initially a compileable refusing import skeleton exposes absent behavior as assertion failures.
- [x] Run `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml --lib composition_graph -- --test-threads=1`; expected behavioral failures for valid imports, no compile errors.
- [x] Implement preflight over borrowed objects with depth 64, 100,000 aggregate objects, 1,000,000 aggregate values, 256 MiB aggregate stored payload and 64 MiB individual stored stream limits. Count dictionary keys. Reject before moving into the accumulator. Use small injected limits only in unit tests.
- [x] Allocate a complete deterministic ID map with checked arithmetic, rewrite the consumed input in isolation with cancellation checks, clear obsolete stream offsets, then commit staged objects/maps/usage after one last checkpoint. No destination mutation before that checkpoint.
- [x] Run focused tests; expected all pass. Add exact-limit/one-over and ID-exhaustion regressions where initial tests lack them; verify new behavioral tests before the smallest correction if needed.

## Task 2: Evidence and review

**Interfaces:** Consumes Task 1 source and tests; produces recorded focused/library terminal results, exact source fingerprints and continuation status.

- [x] Run the ordinary native library suite (same offline environment); expected all nonignored tests pass. No UI/build/portable claim for this unwired primitive.
- [x] Request one fresh scoped implementation review of primitive/tests/design dispositions. Resolve demonstrated important defects with regression tests; no routine re-review.
- [x] Record terminal counts, preserved limits, absence of Save integration and next parser/catalog/lineage work in the execution ledger and combined-work continuation. Broad goal remains active.

## Following plans, not enabled by this one

Parser/serialization admission and cancellation; graph-native catalog/structure reconciliation; logical editing lineage and explicit occurrence policy; annotation/form/link adapters; then unchanged four-page desktop baseline and actual combined edits/save/reopen. The five builder findings remain release integration gates; this independently testable primitive does not claim to close them.
