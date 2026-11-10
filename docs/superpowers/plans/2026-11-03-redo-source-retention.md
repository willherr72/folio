# Redo source retention plan
> Execute inline with superpowers:executing-plans.
**Goal:** Reproduce and resolve applied PDF sources surviving discarded Redo branches.
**Architecture:** First bind actual desktop ownership evidence; then introduce release only after workspace/history and queued-recovery owners are absent.
**Tech stack:** React/TypeScript, Tauri, existing native desktop harness.
**Spec:** docs/superpowers/specs/2026-11-03-redo-source-retention-design.md
- [x] Reproduce23Apply/Undo/Redo/Undo branches with exact content, native close acknowledgements and owned-process memory samples.
- [x] Trace every remaining source owner and write failing reachability/recovery-lifetime regressions.
- [x] Implement safe abandoned-source release, preserving available history and asynchronous recovery.
- [x] Verify focused/full frontend/build, corrected desktop lifecycle and independent saved-output fidelity; one integrated review and checkpoint.
Review focus: discarded redo versus available undo; async queued recovery; shared adapters/tabs; stale async work and StrictMode; partial close failure/ownership races.
