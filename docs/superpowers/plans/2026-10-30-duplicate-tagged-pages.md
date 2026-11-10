# Duplicate tagged pages implementation plan

> Execute inline using superpowers:executing-plans. Existing isolated worktree; preserve all inherited work, original inputs, failed evidence and packages. No commit or release.

**Goal:** Save repeated tagged-page occurrences with independent ownership and faithful navigation/content.

**Spec:** `docs/superpowers/specs/2026-10-30-duplicate-tagged-pages-design.md`

## Review focus

- Do aliases, cyclic resource containers or shared form owners force accidental whole-source copying or cross-copy ownership?
- Are original stored bytes and every copied stream charged before cloning, cumulatively across copies?
- Can page/annotation/structure backreferences or ID/Header relationships point at the wrong occurrence?
- Can self-link retargeting mutate another occurrence, capture the other namespace, or alter remote actions?
- Can occurrence-token normalization silently discard an explicit bookmark move to a copy?
- Are original logical order and copied subtree order explicit and independently verified?

## Task 1: Reproduction and bounded copy/selection foundations

- [x] Add and observe a public tagged duplicate Save regression failure.
- [x] Extract read-only bounded structure selection planning, preserving existing selection tests and clone-specific ID semantics.
- [x] Implement a sparse occurrence dependency copier with typed boundaries, alias/cycle preservation and pre-copy cumulative retained accounting. Add meaningful red/green tests for shared immutable payloads, marked owners, byte limits and cancellation.

## Task 2: Independent structure occurrences

- [x] Stage Page/annotation/structure/marked-owner identities and filtered K/ParentTree/IDTree for each copy.
- [x] Merge structure ownership once for original plus all copies; preserve resource/appearance sharing where unchanged.
- [x] Verify sparse slots, shared forms, annotation relations, inherited geometry, independent transforms and retained semantic guards.

## Task 3: Copy-local navigation and Save integration

- [x] Extend existing Prepared/Walker with copy-local named bindings and typed self-target retargeting; preserve actions/remote operands and cached depth.
- [x] Admit repeated pages in structured Save using occurrence-distinct internal tokens and first-occurrence current-source bookmark mapping.
- [x] Defer compatible bookmark writing until output identities exist without bypassing original native metadata/language guards.
- [x] Verify public Save, permissions, cancellation, stale intents, explicit copy bookmark targets and save/reopen.

## Task 4: Authored outputs and desktop acceptance

- [x] Run affected composition/bookmark/Save/link/protection native checks; exact authored output comparisons.
- [x] Independent reader ownership/navigation/pixel/text audits and damaged-output controls.
- [x] Fresh GUI/CLI build and owned Windows Duplicate/Undo/Redo/Save/reopen plus independent-copy adjustment; audit resulting PDFs and stop only the owned process.

## Task 5: Review and checkpoint

- [x] One fresh final implementation reviewer. Resolve concrete findings locally with regression evidence; no re-review churn.
- [x] Update evidence, roadmap and continuation with accurate passing/ignored counts, fingerprints and all remaining limits. Keep the broad goal active.

Corrected checkpoint: `artifacts/duplicate-tagged-pages-20261030/checkpoint-evidence.json`. Both review findings reproduced and corrected; empty-node ownership explicitly ruled. Corrected native gate, independent Header/ID audit and damaged-output controls, build-2 and desktop-2 passed. 228 passing executions / 225 distinct passed tests; eight remaining unexecuted ignored cases. The pre-review evidence and audit-reader failure remain preserved. No release or broad parity claim.
