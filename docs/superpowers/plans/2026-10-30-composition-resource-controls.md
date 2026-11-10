# Composition resource controls implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Give graph-native composition bounded sequential input and cooperative cancellation, and make existing final Save serialization responsive to cancellation.

**Architecture:** Extend the existing opt-in parser scope, feed its remaining allowances through CompositionGraph, and wrap BoundedWriter. Preserve ordinary loader and publication behavior.

**Tech Stack:** Existing Rust/lopdf/PdfSaveControl; no dependency changes.

**Spec:** `docs/superpowers/specs/2026-10-30-composition-resource-controls-design.md` and parent structured-composition design.

## Global constraints

- Preserve the complete-source fast path, structured Save guard, originals, dirty work, prior candidates and failed evidence. No commit/push/release/app restart.
- One heavy offline locked native workload, one Cargo build job, existing shared target.
- Cooperative checkpoints, logical resource budgets, no hard latency or RSS claim.
- No unsafe lifetime extension, polling helper, renderer substitution, or publication before existing final commit.

## Review focus

- Cancellation probes must not run under RefCell borrows or survive their load scope.
- Nested loads/errors/unwinds must not reset or leak a caller's scope.
- Budget remainder must accumulate input/decoded/object/count allowances across successful sources.
- Cancellation after parser success but before graph commit must preserve graph and allowances.
- Writer errors/cancellation must never expose partial bytes or change successful output.

## Task 1: Controlled bounded input

Files: vendor `lopdf-0.45.0/src/load_limits.rs`; new `src-tauri/src/composition_io_tests.rs`; test module registration.

Interface: `Document::load_mem_with_resource_control(buffer, limits, cancelled: Arc<dyn Fn() -> bool + Send + Sync>) -> Result<(Document, ResourceLimits)>`. Existing bounded method stays intact.

- [x] Add a compileable delegating skeleton and red tests for pre/mid-load cancellation, remaining allowances, classic/modern fidelity and recovery after probe panic. Run `cargo test --lib composition_io -- --test-threads=1` offline/locked; expected behavioral failures.
- [x] Extend scope state with optional probe; charge/check use sticky cancellation outside borrows. Return remaining allowances on success and decrement input bytes. Add periodic long-token checks. Run focused tests and existing redaction_loader tests; expected pass.

## Task 2: Transactional bounded graph entry

Files: `composition_graph.rs`, `composition_graph_tests.rs`.

Interface: `CompositionGraph::import_bytes(source_key, bytes, cancelled)`; existing relocation becomes module-private. Keeps a private `ResourceLimits` remainder.

- [x] Add red tests for actual serialized sources with conflicting IDs, cumulative input/decoded allowance refusal, and cancellation leaving graph plus parser allowance unchanged.
- [x] Implement sequential controlled parse followed by relocation and final allowance commit. Run graph + I/O + existing loader tests; expected pass.

## Task 3: Controlled final serialization

Files: `pdf_output.rs`, `complete_source_save.rs`, final serialization call in `engine.rs`; unit tests in pdf_output.

Interface: `save_controlled(document, Option<&PdfSaveControl>)`; internal checked writer helper permits deterministic cancellation tests.

- [x] Add red tests that cancel during large writes and retain the cancellation error; compare successful checked/ordinary serialized bytes. Test pre-cancel, final check and sticky failure. Expected failure before implementation.
- [x] Implement bounded chunk checks and wire both existing final serialization sites. Run focused writer tests, full library, `--test pdf_save --test complete_source_save`; expected all nonignored tests pass, original/destination guards retained.
- [x] Freeze source/test/spec hashes; one final implementation review, address demonstrated important findings with regressions, record terminal evidence and remaining catalog/lineage gates. No full suite or desktop claim beyond executed checks.
