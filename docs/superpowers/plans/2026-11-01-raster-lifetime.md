# Persistent page-raster experiment plan

> Execute inline with superpowers:executing-plans; one fresh final scoped review.

**Goal:** Test a rendering-lifetime hypothesis against real pixels and memory.
**Architecture:** Captured DOM display prototype in an owned verified 0.27 process;
unchanged native edit/preview/cancel flow and birth-bound memory collector.
**Tech stack:** Playwright, Canvas/SVG, PowerShell, Python/Pillow.
**Spec:** docs/superpowers/specs/2026-11-01-raster-lifetime-design.md

## Constraints and review focus

Preserve package/source/evidence. Never force GC or relax original budgets. Check
superseded-image races, actual canvas readiness, screenshot comparison neutrality,
same-coordinate transforms and pointer transparency, fixture/runtime provenance.
No product correctness claim from this injected prototype alone.

### Task 1: Build the isolated probe

- [x] Capture fixture before launch and archive all runtime helpers.
- [x] Implement bounded persistent-surface ownership and actual readiness checks.
- [x] Verify syntax, package identity and independent PNG comparison controls.

### Task 2: Execute the discriminator

- [x] Run stock and persistent-canvas constant-content sessions to completion.
- [x] Compare exact displayed pixels, source restoration and memory budgets.
- [x] Run varying content if the first pair supports the hypothesis; otherwise
  preserve negative evidence and select the next justified boundary.
- [x] Test a bounded two-slot SVG lifetime alternative if canvas resampling differs;
  require the same exact pixel and resource comparisons, with captured inputs.

### Task 3: Reconcile and review

- [x] Record raw evidence, all failures and the next production/diagnostic action.
- [x] Obtain one scoped independent review and address important findings.
- [x] Update roadmap and continuation; leave overall quality goal active.
