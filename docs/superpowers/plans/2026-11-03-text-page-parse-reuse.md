# Text-page parse reuse implementation plan

> Execute inline with superpowers:executing-plans in the existing isolated worktree. One fresh final review; no per-task delegates. Preserve evidence and unrelated changes; no commits or publication.

**Goal:** Remove duplicate strict parsing during existing-text page preparation without weakening fidelity or refusal behavior.
**Architecture:** Share one bounded SourceForm through resource and form preparation; retain existing request-local lifetime.
**Tech Stack:** Rust, lopdf, PDFium, PowerShell.
**Spec:** `docs/superpowers/specs/2026-11-03-text-page-parse-reuse-design.md`.

## Constraints and review focus

Same parser policy and bounds; one immutable source; exact single-page mapping; separate isolated/shared-field behavior; no cross-request retention. Existing permissions, graph validation and group/publication checks remain. Source and renderer fingerprints qualify every run.

## Implementation and validation

- [x] Snapshot baseline files and add an actual worker-route parse-count regression in `text_context_projection_tests.rs`, with a test-only source-specific counter in `acroforms.rs`. Observe duplicate parses before changing production behavior.
- [x] Refactor internal form projection helpers in `acroforms.rs` to consume a validated single-page SourceForm. In `text_edit.rs`, enable reuse once and pass that same source through resources and forms. Keep every existing guard.
- [x] Add form/resource combinations, invalid mapping and uncached/refusal coverage; run focused and full library tests.
- [x] Run affected text, form, complete-source and resource integration suites; obtain one final source review and fix demonstrated defects with focused regressions.
- [x] Record exact scope, counts, limitations and next runtime/desktop gate in persistent artifacts and quality ledger. Do not claim quantified performance or release parity from parse counts.

Runtime acceptance extension: after optimized integration, use the unchanged public-engine workload with checkpoint-bound old and current optimized binaries in fixed ABBA order. Pin identical fixture/test/renderer bytes and the exact three changed source paths. Audit all four runs against original and accepted PDFs, including direct glyph checks on unchanged pages; reject wrong-role/binary/output bindings. This is a sequential one-host observation, not randomized causal attribution or GUI acceptance.

Completed native milestone: final-summary.json and findings.md record all gate results, build-method ruling, review correction, coverage limits and pending desktop acceptance.
