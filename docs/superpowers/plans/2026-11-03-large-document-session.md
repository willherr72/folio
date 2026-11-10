# Large-document desktop session implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline, with one final fresh-context review.

**Goal:** Find and correct practical resource/fidelity failures beyond the small-document workflows.

**Architecture:** Reuse the verified optimized package, raw CDP controller and identity-bound process sampler. Add an explicit larger-document workload and independent source/output auditing. Keep observations and acceptance claims separate.

**Tech Stack:** Existing Tauri/Rust/React application; Node raw CDP, PowerShell process ownership, Python MuPDF/pypdf audits.

**Spec:** docs/superpowers/specs/2026-11-03-large-document-session-design.md

## Global constraints

- Preserve original documents, accepted packages, failed attempts and unrelated worktree edits.
- No CDP Network observation, emulation, tracing or forced GC in resource sessions.
- The 6 GiB operational stop is not a quality acceptance budget.
- No commits, remote release or user-editor restart.

## Work

- [x] Bind package, source, fixtures and per-page independent expectations; inspect current navigation/editing interfaces.
- [x] Implement raw-controller tab/navigation/edit/save/close session and capture complete failure receipts and owned cleanup. Run on the unchanged accepted package (preserved failing runs establish the scan composition defect; no full-session pass).
- [x] Audit source identity, navigation/revisit text and PNG identity, history and saved-PDF fidelity. Exercise corruption controls for the audit's consequential claims.
- [x] Diagnose any observed failure. Correct product behavior only with reproducing regression and fresh affected validation.
- [x] Obtain one final review, reconcile exact evidence and limitations, update the quality roadmap and keep the full goal active.

## Review focus

Check stale-page admission during navigation, text/glyph whitespace normalization limits, source-versus-output identity, memory observer effects, cleanup failures, and overclaiming cache bounds or larger-workload performance.

## Demonstrated failure work

- [x] Reproduce native large encoded-source admission mismatch before changing product code.
- [x] Set explicit composition input bytes to the existing source cap; preserve all allocation, decode, object and graph limits.
- [x] Run library and affected save/render tests; build a separate optimized GUI candidate with exact source binding.
- [x] Corrected candidate must pass isolated scan rotation/save/reopen before the whole workload. No existing portable is overwritten.

Final validation: 1,451 library tests passed (10 ignored), and 37 affected integration tests passed (1 ignored). Native-1/2 linker failures remain preserved; native-3 passed after free disk space returned. Fresh TypeScript/Vite and optimized GUI builds passed with unchanged source binding. Corrected rotation-smoke-3 and session-6 passed: 140 observations, 21 edits, 30 resource samples, independent audits of all 340 saved pages, and 11 rejected corruption controls. All 45 observed identities across the preserved attempts are absent. The eight audit helper tests and prelaunch renderer-mismatch control also pass. R1–R3 are closed by parent corrections and this evidence, not a second review. See final-summary.json, review-disposition.json, and checkpoint.json under the milestone artifact directory. Resource results are observations only; the overarching goal remains active.
