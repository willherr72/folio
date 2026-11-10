# Source form admission reuse implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox syntax for tracking.

**Goal:** Reduce repeated form admission while preserving editing fidelity and authorization.
**Architecture:** A small positive-only two-mode cache owned by each OpenDocument; permission checks precede cache access. Existing validators execute unchanged on a miss.
**Tech Stack:** Rust, OnceLock, existing PDFium/lopdf worker and native tests.
**Spec:** `docs/superpowers/specs/2026-11-03-form-admission-reuse-design.md`

## Global constraints

No permission bypass, negative caching, cross-source sharing, retained parsed graphs, relaxed limits, or publication-check removal. Preserve unrelated work, accepted binaries, original PDFs and all failed evidence. No commits or release publication in this milestone. Use D: native cache/TEMP.

## Review focus

- Every OpenDocument creation path starts empty, including private previews.
- A permissive annotations-mode success cannot authorize strict mode.
- Current protected/print-only policy remains authoritative after success.
- Repeated refusal must execute validation and retain original errors.
- New/closed source identities cannot inherit a previous success.

### Task 1: Bounded positive reuse and regression proof

Files: create `src-tauri/src/form_admission.rs`, `src-tauri/src/form_admission_tests.rs`; modify `engine.rs`, `form_text.rs`, `styled_paragraph_publish.rs`, `text_edit.rs`, `text_group.rs`, `text_paragraph.rs` to initialize each source's cache.

Interface: `form_admission::Admission::check<E>(&self, annotations: bool, validate: impl FnOnce() -> Result<(), E>) -> Result<(), E>`. Only successful calls skip the callback next time in the same mode. Authorization is the caller's responsibility, checked first.

- [x] Write standalone tests for repeated success, separate modes, repeated failure/retry, independent instances, and panic recovery. Run against a pass-through implementation and retain the behavioral failure.
- [x] Implement success-only storage; rerun the same tests.
- [x] Integrate with all constructors and `form_source_admission`; add native tests proving mode/refusal/source/policy behavior through the real worker and actual PDF fixtures. Native execution remains in the next unchecked step.
- [x] Run native focused tests and affected suites with source/test fingerprints. Diagnose failures before acceptance.
- [x] Measure the same two-visible-page discovery workload on 2/300-page sources and audit edited output fidelity; distinguish first-call cost from reuse.
- [x] Obtain one final independent source review; no production defect found. Record the minor valid-page test-coverage gap and unjudged runtime gates in `artifacts/form-admission-reuse-20261103/review/review.md`.
- [x] Record exact evidence and unresolved parity gaps in the persistent quality ledger. Keep task incomplete until required checks finish.
