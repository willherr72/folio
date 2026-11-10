# Text-discovery diagnostic implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Execute inline; preserve the existing worktree and unrelated work.

**Goal:** Identify current ordinary-text discovery costs without weakening fidelity checks.
**Architecture:** Reuse the bounded span collector and temporary feature-gated diagnostic binary. Extend the persistent worker workload with exact first/repeated discovery comparisons. Restore all instrumented product bytes after execution.
**Tech Stack:** Rust, PowerShell, Python, PDFium/lopdf and independent MuPDF checks.
**Spec:** `docs/superpowers/specs/2026-11-03-text-discovery-profile-design.md`

## Constraints and review focus

- No persistent product behavior changes, permission bypass, parser-policy changes or lost refusal checks.
- Same source version for each first/repeat comparison; subsequent edits legitimately change source identities.
- Call the occurrence-form handler, not its legacy implementation.
- Bound timestamps and nesting; do not add overlapping inclusive medians.
- Bind outputs to original fixture hashes and independently accepted desktop references.
- Restore only exact instrumented versions; refuse to overwrite concurrent edits.
- Keep accepted GUI/portable binaries, original PDFs and failed evidence. No commits or publication.

## Diagnostic task

Artifacts: `artifacts/text-discovery-profile-20261103/{instrument.py,clock.rs,workload.rs,main.rs,run.ps1,analyze.py,check-accounting.py}`. Temporary source scope is declared by `instrument.py`; no changes beyond that manifest are allowed.

- [x] Pin existing diagnostic helpers and accepted desktop inputs; write the extended workload and repeated-discovery receipt checks.
- [x] Exercise clock/accounting controls, install exact temporary spans, and build/run the optimized diagnostic using D: build and temporary caches.
- [x] Restore source bytes and verify the full production fingerprint, including on diagnostic failure.
- [x] Audit all 602 pages and timing/equality/reference controls. Obtain one final source review if the diagnostic delta introduces substantive interpretation risks; do not delegate small corrections.
- [x] Record measured stage costs, a comparison visualization, limitations and the next justified optimization in the persistent ledger.

Completed: qualified build-3/run-2 and all-page audit accepted. See diagnostic findings and final-summary.json. No production optimization was introduced.
