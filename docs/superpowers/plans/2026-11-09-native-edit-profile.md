# Native editing subphase diagnosis implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline; one final fresh reviewer of the completed diagnostic.

**Goal:** Attribute large-PDF editing latency to actual worker subphases before selecting an optimization.
**Architecture:** Default-disabled scoped spans, a bounded scalar-only collector and a real optimized PdfEngine example. Matched unobserved runs share its executable and unchanged editing paths.
**Tech stack:** Rust/std, existing Cargo/serde_json, Python/PyMuPDF; no new dependencies.
**Spec:** docs/superpowers/specs/2026-11-09-native-edit-profile-design.md

## Constraints and review focus

Preserve036/launcher/originals; serialize builds and workloads. No policy/cache/history optimization yet. No normal-build clocks/locks/allocations at span sites.4096 records/depth128; exclusive recording and one admitted worker thread. Fixed labels/scalars only. Out-of-order/unclosed/wrong-thread/overflow invalidates profiling. Nested durations require direct-child subtraction. Compare full source-bound saved output, not merely a matching selected word. Do not promote the diagnostic executable.

### Task1: bounded recorder and instrumentation

Files: create src-tauri/src/native_profile.rs and native_profile_tests.rs; feature/macro in Cargo.toml/lib.rs; timing sites in engine.rs, text_edit.rs, text_context_edit.rs and occurrence_text_edit.rs. Record baseline source/native inputs and checkpoint first.
Interfaces: `start()->Result<Session,&'static str>`, `Session::finish()->Report`, `span(&'static str)->Span`. Report carries `valid`, `errors`, `elapsed_ns`, `records`; records carry id/parent/label/start_ns/end_ns. Guards do not wrap or change operation return values. Macro `native_phase!(literal)` returns Span when enabled and unit otherwise.

- [x] Write standalone std-only recorder tests and preserve missing-module RED. Cover nested parents/monotonicity, disabled recording, concurrent start, errors/unwind cleanup, stale guards, cross-thread admission,4096-record/depth128 bounds and invalid unfinished scope.
- [x] Implement recorder GREEN with rustc --test (single test thread; tests manage own child threads), no new dependency. Add exact cfg/macro and scoped phase sites without changing original operation order or guard policy. Check default/enabled example compilation.

### Task2: real optimized worker workload and independent analysis

Files: src-tauri/examples/native-edit-profile.rs; artifacts/native-edit-profile-20261109/{prepare.py,run.ps1,audit.py,analyze.py,test_analysis.py}.
Interfaces: example accepts source/dll/output-root/observe parameters, writes operation records and final PDF using exclusive new files. `prepare.py` binds baseline/source/renderer/helper/fixture hashes. Analysis verifies interval tree, computes inclusive/exclusive timings and compares repeated operations; saved audits compare all pages to source/established golden and control/observed output.

- [x] Implement the seven-change public-worker driver, retaining sources, checking discovered expected text and final save. Bind bytes/hashes and optimized feature profile. Run control300, observed300, control2, observed2 serially, preserving failed attempts.
- [x] Add analysis negative controls before analyzer implementation: reject missing/unclosed/overlapping/cross-parent/overflow events; never count nested durations twice. Require operation and engine success plus unchanged fixture/helper/source hashes.
- [x] Independently audit all control/observed saved glyphs/boxes/paint at72/144dpi, changed-page golden and unrelated originals. Report dominant stages, observer effect and limitations without a generalized speedup claim.

### Task3: review and diagnostic checkpoint

- [x] Freeze actual scope/evidence and request one review. Correct consequential findings inline, rerunning affected checks only.
- [x] Seal source/helper/binary/fixture/trace/audit evidence. Recheck accepted036 historical checkpoint and unchanged launcher; record next optimization justified by measured evidence. Keep broad goal active.


Completed diagnostic: default/enabled checks and corrected optimized build3;28 immutable edits over4 native runs;604 saved pages independently equal source/golden glyphs, geometry and full72/144dpi RGB;11 recorder regressions and15 analyzer controls. Review finding corrected exact UTF-8 text, with fresh build/runtime evidence.3289 checkpoint pins verified. No default-package speedup or full native/frontend/GUI/parity claim; portable036/launcher unchanged. Next split source publication into parse/graph/serialization and evaluate bounded request-local reuse.
