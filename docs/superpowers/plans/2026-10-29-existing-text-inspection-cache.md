# Existing text inspection cache implementation plan

> Execute inline using superpowers:executing-plans; one final scoped reviewer.

**Goal:** Avoid repeated inspection when returning to unchanged PDF text.
**Architecture:** Small bounded cache of merged immutable-source inspection, with
source lifetime invalidation and defensive copies. Native edits remain revalidated.
**Tech stack:** TypeScript/React, Vitest, existing Windows acceptance harnesses.
**Spec:** docs/superpowers/specs/2026-10-29-existing-text-inspection-cache-design.md

## Constraints and review focus

32entries/2MiB estimated payload; exact adapter/source/page/readers; pending eviction,
late invalidation, partial errors, capability changes, defensive copies and source
close races. No source PDF edits, package overwrite, commits or parallel Cargo.
Existing isolated worktree remains authoritative; capture baseline before edits.

## Implementation and unit verification

Files: new src/editor/existing-text-cache.ts; modify ExistingTextLayer.tsx,
PageView.tsx, adapter.ts; new tests/existing-text-cache.test.ts and
existing-text-cache-ui.test.tsx.

- [x] Reproduce remount duplicate inspections with a failing UI test.
- [x] Add cache tests for isolation, mutation, failure/retry,32entry/2MiB eviction,
      pending clear/eviction, workspace cleanup and native close.
- [x] Implement readExistingTextRuns(adapter,sourceId,pageIndex):Promise<TextRuns>
      and clearExistingTextCache(sourceIds?:readonly string[]):void; integrate.
- [x] Run targeted tests, full frontend suite and production build.

## Native desktop acceptance and review

- [x] Build fresh source-bound GUI/CLI local candidate without replacing old output.
- [x] Repeat archived-helper Word/mixed Preview/Cancel loops, compare exact-source
      inspection counts with prior trace; no stale hit targets after changed source.
- [x] Run corrected narrowed Word Apply/history/save/reopen and independent audit.
- [x] One final scoped review, one correction pass, exact evidence checkpoint.

Artifacts: artifacts/existing-text-inspection-cache-20261029.
