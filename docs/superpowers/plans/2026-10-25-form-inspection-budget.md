# Form inspection allocation budget plan

Use superpowers:executing-plans inline; standing authorization removes routine
approval stops. Existing isolated worktree; baseline is artifacts/form-inspection-
budget-20261025/baseline, not Git HEAD. One final scoped reviewer, no rereview.

## 1. Reproduction and bounded decoding

- [x] Add unit regressions for preallocation refusal, exact Unicode decoding,
      invalid/moving native payloads and aggregate/shared-string budgets.
- [x] Add public API fixture with shared options that expands beyond 8 MiB,
      verifies original bytes and continued use of a normal second page.
- [x] Observe red evidence before changing implementation.

## 2. Implementation and regressions

- [x] Implement budgeted string decoding and owned line-ending normalization.
- [x] Replace unlimited enumeration/single-field inspection; account vector
      capacities, fallback labels, selection copies and small owned strings.
- [x] Run focused and existing form authoring/filling/export/protection suites.

## 3. Review and accepted checkpoint

- [x] Freeze scope/hashes/delta, request one scoped review, reproduce/fix findings.
- [x] Build isolated Windows executable; verify oversized-data refusal, subsequent
      normal page/form use, filling, save/reopen and independent output fidelity.
- [x] Record exact executable/source/renderer hashes, preserved sources/failures,
      cleanup, limits and next gaps in the persistent quality ledger.

Review focus: charging before allocations, checked arithmetic, UTF-16 edge cases,
vector capacity vs length, repeated/fallback string copies, selected-index aliases,
finite limits in all callers, filter-before-read ordering, error cleanup and source
fidelity. Existing PDFium/parser/source/render memory remains outside this model
budget; do not infer a process-wide ceiling from passing narrow tests.
