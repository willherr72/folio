# Multi-page text preview implementation

Spec: docs/superpowers/specs/2026-10-29-redaction-preview-design.md

Execute inline in the existing isolated worktree. Preserve all prior artifacts, unrelated edits, original PDFs and packages. No public API registration, GUI build, publication or commit. The design review found no consequential open questions.

## Task 1: Source-owned multi-page marks
Add explicit page geometry to opaque selections/plans, overlapping mark union, per-mark hit validation, and one source-bound batch kernel call. First add failing tests for discontiguous/overlapping marks and pages with repeated block ordinals. Then implement and run focused redaction library tests.
Expected: new tests fail before implementation and all focused tests pass afterward.
Interface: plans retain source identity/hash, page geometry, ranges and measured boxes; preview consumes these internally only.

## Task 2: Worker-owned preview transactions
Add crate-private request/response types and registered owner sessions, a bounded preview store, Apply/Discard/bounds/release, expiry and document-close hooks. First run lifecycle tests against refusing stubs; implement ownership checks and cleanup. Include wrong owner, late owner, byte/allocation mismatch, dropped responses, source/candidate closure, once-only adoption and count/byte/geometry limits.
Expected: successful workflows fail with stubs, then pass with unchanged original sources and no orphan candidate registrations.
Interface: the worker owns pending candidate allocations; successful Apply transfers ownership. No client-provided source plans or owner registration.

## Task 3: Independent acceptance and review
Export actual adopted multi-page candidates; verify both readers' retained text, positions and pixels using source-only selection masks, plus decoded graph absence. Run focused and library checks, snapshot source/evidence, obtain one fresh final review, correct consequential findings locally with regression tests and record exact limitations.
Expected: complete passing evidence and a persistent checkpoint, while public UI and hostile-parser acceptance remain explicitly pending.

## Review focus
Allocation ownership versus IDs; wrong-owner cleanup; post-release queued requests; expiry; dropped successful responses; aggregate accounting; nonrecursive closure; duplicated marks and page-local block indices; original path protection; source-only audit masks and untouched pages. Parser per-stream bounds are not an aggregate memory guarantee.
