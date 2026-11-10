# Original-page read exports implementation

Spec: `docs/superpowers/specs/2026-11-02-original-page-exports-design.md`
Ledger: `.superpowers/sdd/2026-11-02-original-page-exports/progress.md`

## Task 1: Shared private read-page route
- [x] Add integration regressions for orphan widgets and hybrid XFA static appearances in text/image export. Run them before product changes; expect current form-preservation errors.
- [x] Add bounded single-source graph retention and selected-page overlay attachment without relaxing existing Save/occurrence contracts.
- [x] Add internal read-page preparation with unchanged-byte reuse, exact annotation receipts, flattening for text, crop/rotation, units, and permissions. Integrate text/image/preview consumers.
- [x] Run focused tests and affected export/overlay/security/OCR regressions; expect terminal passes and unchanged sources.

## Task 2: Real-producer and integration acceptance
- [x] Build a local CLI candidate, keeping portable 0.31 unchanged. Run the retained IRS/NIST/quiz corpus through text/image exports; compare independently and record hashes and limitations.
- [x] Run the broader native gate once source is stable. Record every failure, ignored case, and any environmental limitation.

## Task 3: Integrated review and checkpoint
- [x] Freeze scoped before/after source and evidence. Request one fresh integrated review using the executing-plans workflow.
- [x] Resolve consequential findings with reproductions and targeted verification; record dispositions and an honest checkpoint. Leave the overall goal active.

## Review focus
Selected page indexing in multi-page documents; imported annotation deletion and layering; repeated leaf/shared annotation aliases; permission/cancellation bypasses; graph/resource budgets; static XFA versus dynamic XFA claims; OCR and optional password field values; source and accepted-package immutability. No per-task review agents or re-review loop.

Final correction pass has targeted RED/GREEN evidence for both P2 review findings and exact annotation-slot deletion. native-full-2 passed 2,604 tests with zero failures and 28 explicit skips. Final CLI corpus passed six operations and 52 exact original-render image comparisons; seven strict cross-renderer differences remain recorded. Checkpoint: artifacts/original-page-exports-20261102/checkpoint.json. Earlier evidence is preserved separately.
