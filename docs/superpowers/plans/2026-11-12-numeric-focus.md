# Precise numeric focus implementation plan

> **For agentic workers:** Use superpowers:executing-plans for inline execution. Steps use checkbox syntax.

**Goal:** Restore replacement typing when keyboard focus expands precise numeric drafts.

**Architecture:** Keep the native number input and synchronous expansion. Restore full-value selection for non-pointer entry; preserve pointer caret behavior and consumer event handlers.

**Tech Stack:** React, TypeScript, Vitest, actual Windows Tauri WebView.

**Spec:** docs/superpowers/specs/2026-11-12-numeric-focus-design.md

## Global constraints

Preserve exact owner drafts, numeric semantics, current selected 048 build and dirty worktree. No commit, deletion, release, version bump or private source exposure. Retain evidence and failures. Execute inline with one final combined review.

## Review focus

- Pointer cancellation and prevented default must not leak intent to subsequent keyboard focus.
- Pointer focus must not force full selection or alter caller pointer callbacks.
- Focus callbacks must see expanded values and retain selection override control.
- Repeated focus/blur must emit no edit and leave exact state intact.
- Native keyboard behavior must be verified on new executable bytes, not inferred from jsdom.

## Task 1: Fix the component

- [x] Retain exact component/test baselines and prior Windows RED.
- [x] Add tests for select-after-expansion ordering, pointer preservation, intent clearing and handler composition.
- [x] Run focused tests and observe missing-selection failure.
- [x] Implement local pointer-aware selection repair in PreciseNumberInput.tsx.
- [x] Run focused tests, full frontend suite and TypeScript/frontend build. Expected: all pass.

## Task 2: Windows verification and review

- [x] Capture current product/native/resource fingerprints; build one fresh normal optimized GUI component candidate. Reuse unchanged native correctness evidence explicitly; no new full-native claim.
- [x] Use isolated owned Windows session and public PDF to repeat actual keyboard RED cases; add pointer/caret, exact draft and bounds checks. Expected: replacement works, pointer appends at chosen caret and callbacks do not synthesize edits.
- [x] Request one final scoped combined review with baseline, source, tests and runtime evidence. Address important findings with RED/GREEN, preserve minor dispositions.
- [x] Record completion and remaining portable integration work. Keep selected 048 unchanged until separate verified portable replacement.
