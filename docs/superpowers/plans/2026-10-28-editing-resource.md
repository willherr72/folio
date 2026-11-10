# Sustained paragraph editing and resource plan

> Execute inline using superpowers:executing-plans. One final scoped reviewer.

**Goal:** Measure repeated editing responsiveness/cleanup and prove useful narrow reflow.
**Architecture:** Owned Windows processes, passive CDP observation, private-memory
snapshots, bounded repeated UI operations and independently audited saved output.
**Tech stack:** Existing Playwright/PowerShell/Python tools and current Folio candidate.
**Spec:** docs/superpowers/specs/2026-10-28-editing-resource-design.md

## Constraints/review focus

No forced GC, no original modification, no reused evidence directory. Verify exact
process identity; count successful closes by candidate ID; separate debug timings
from optimized/hardware claims. Check observer blind spots, asynchronous cleanup,
memory warmup/allocator effects, retained drafts after refusal, and genuine reflow.

## Task 1: Sustained workflow

- [x] Implement harness/resource collector with machine and identity binding.
- [x] Run3warmup+20measured cycles on Word and mixed/highlighted PDFs.
- [ ] Diagnose budget or lifecycle failures; add failing regression before product fixes.

## Task 2: Narrow reflow and acceptance

- [x] Establish a Word body width/height that increases line count safely.
- [x] Exercise refusal/draft preservation, narrow reflow, Apply, Undo/Redo and reopen.
- [x] Audit complete output text, glyph/style/baseline, surrounding pixels and tags.
- [x] Obtain one scoped review, correct important findings once, reconcile all evidence.

Artifacts: artifacts/editing-resource-20261028. Keep the full goal active.

Functional acceptance and review correction are complete; full resource diagnosis remains open. The narrow case intentionally uses lineSpacing1.0 to preserve neighboring Status text. See checkpoint-functional.json for terminal evidence, old-run provenance limitations and the retained threshold failure. No product changes or parity claim.
