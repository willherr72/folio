# Bounded request parse reuse implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline, with one final fresh-context review.

**Goal:** Reduce actual native text-edit latency while retaining all validation and existing history/memory behavior.
**Architecture:** Exact-byte/policy-keyed immutable parse templates in a bounded thread-local request scope. Inspections borrow shared read views; mutable callers receive independent owned Documents. No persistent history cache or new dependency.
**Tech stack:** Rust/std, patched lopdf, existing optimized native/Windows harness and independent Python reader.
**Spec:** docs/superpowers/specs/2026-11-09-request-parse-reuse-design.md

## Constraints and review focus

Preserve036/launcher and sealed diagnosis. UTF-8 explicit for every source edit. Serialize compiler/suite/desktop/resource work. Keys exact bytes plus strict64/compatible64/compatible16 policies; preserve original parser/error/source limits and every downstream guard.2entries,64KiB minimum/8MiB aggregate encoded input,16MiB charged model,100000 actual PDF Object values, nesting64. No cache for errors/encryption/bookmark state. All100 Undo/Redo actions and predefined memory/fidelity targets remain. Review immutable clone isolation, stale scope insertion, policy crossover, aggregate admission before clone, unchanged loader options and proof of actual speedup.

### Task1: bounded cache and equivalent loader routing

Files: src-tauri/src/request_parse.rs, request_parse_tests.rs; lib.rs module; pdf_output.rs/acroforms.rs/bookmarks.rs/form_graph.rs/physical_units.rs/text_context_group.rs loader calls; text_edit.rs/occurrence_text_edit.rs scope guards.
Interfaces: `Policy::{Strict64,Compatible64,Compatible16}`, `read(&[u8],Policy)->lopdf::Result<ReadDocument>`, `ReadDocument::into_owned()->Document`, `Scope::enter()->Scope`. Scoped internal `read_with`/`load_with` accept a test parser; production uses exact fixed options. Cost walk runs before key retention; immutable models move into Arc storage. Last scope drop clears entries; no active scope means direct parser.

- [x] Capture/verify baseline3365a9a source, native inputs, sealed probe/renderer/fixtures and launcher before edits.
- [x] Create no-reuse API stub and real-parser tests; record functional RED assertions (not a missing-file compile error). Cover all spec input/lifetime/budget classes.
- [x] Implement scoped reuse and watch GREEN, then add equivalent loader wrappers and request guards with UTF-8-preserving unique edits. Compare every option/guard/error mapping to baseline.

### Task2: measured default/native behavior

Files: artifacts/request-parse-reuse-20261109/{build/run/analysis/audit helpers}, copied unchanged probe/runtime harness with provenance.

- [x] Build/check default and profiling configurations and optimized probe with source/example/fixture guards. Fresh before/after control+observed300/2 workloads; complete independent saved-page audit and compare repeated timings with explicit limits.
- [x] Run meaningful affected native regression including source/context/publication, layers/protected/form/resource/geometry policy cases. Build normal desktop candidate and exercise actual existing-text edits, original100-step Undo/Redo and scoped resource/fidelity acceptance, preserving budgets.
- [x] Reject/revise optimization if parse reuse does not produce actual benefit or violates fidelity/retained-memory targets; do not weaken a gate to qualify it.

### Task3: review and qualify next integration

- [x] Freeze actual final source/helper/binary/performance/resource/output evidence for one review; fix consequential findings inline with affected rechecks.
- [x] Seal verified optimization with historical independence; record integration prerequisites and remaining limits. Keep broad goal active and accepted036 launcher until a qualified combined package.


Evidence-led representation amendment: the initial counter charged dictionary keys and xref records as PDF values, rejecting the measured source despite its77,440 actual Object values and11,978,131 charged model bytes. Keys/xrefs/container storage remain fully charged to the unchanged16MiB model cap, with a separate100000xref-record bound. Different policies still independently parse once; only complete bit/order-sensitive equality permits sharing the one immutable template and recording that validated policy. No policy is inferred, failed parse cached, or performance/history/memory gate relaxed. Numeric caps are unchanged; two same-source policy clones no longer consume duplicate model storage.


Read-only architecture amendment after correctly bound performance3 failed: inspection callers receive immutable shared Arc<Document> views, while writable load explicitly materializes an independent owned Document. SourceForm OnceCell stores that read view and exposes only borrowed Cow data. Primary-template retention prevents transient output parsing from evicting the hot original under fixed budgets; unrelated large inputs remain uncached. Scope bounds/error/policy/model equality remain, and caller-owned views can outlive the scope just as prior owned parses could; the cache itself retains nothing after its final guard. Tests now24, including pointer sharing, mutation separation and view lifetime.
