# Reader copy page boundaries implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline under standing autonomous authorization. Preserve the managed dirty worktree; one final scoped review and at most one Important correction pass. No implementer agents.

**Goal:** Copy text across selected PDF pages without merging their boundary words.
**Architecture:** Keep extraction/scalar selection and viewport ownership in PdfTextLayer. Assemble nonempty fragments per selected page occurrence, then preserve or insert a page-boundary LF.
**Tech stack:** React/TypeScript, Vitest/jsdom, current Tauri/Windows WebView harness.
**Spec:** docs/superpowers/specs/2026-11-15-reader-copy-page-boundaries.md

## Constraints

Fresh053 native source freeze must finish before product edits. No OS clipboard probes/operations, private inputs, source-PDF mutation, package/version/launcher change, Git mutation or deletion. Frontend two workers/default5000ms; native/build/UI serial, one Cargo job/existingD:cache. Existing053 archive/checkpoint remain immutable.

### Task1: reader copy model and regression coverage

- [ ] After fresh current native audit terminates, inspect its result/target membership and capture the component baseline.
- [ ] Add focused RED cases at the existing reader-selection test boundary: nonempty distinct/duplicate pages, preserved CR/LF/CRLF, cropped/empty pages, partial surrogate ranges, backwards selection and other-viewport/protected exclusion. Keep single-page text unchanged.
- [ ] Implement the minimal page-fragment assembly in PdfTextLayer (extract a pure helper only if needed to keep the component readable). Run focused GREEN and complete frontend with source/input guards.

### Task2: compiled acceptance and component closure

- [ ] Build normal optimized GUI with26exactresources. Verify real current WebView DOM selection across authored page occurrences through constructed copy events with detached captured data; originals unchanged, owned app stopped. No OS clipboard/physical input claim.
- [ ] Request one final scoped review on the most capable available model; disposition every finding/declined behavior and perform at most one Important RED/GREEN correction pass.
- [ ] Record current source/build/runtime/fresh frontend hashes and exact native-input equality to the fresh prior gate. Update live docs as a separate source component; leave broad goal active and053/launcher unchanged.
