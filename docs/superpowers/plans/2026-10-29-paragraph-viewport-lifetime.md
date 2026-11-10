# Paragraph viewport lifetime implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline in the existing isolated worktree. Include this correction in the one final portable milestone review.

**Goal:** preserve the main document viewport while entering and leaving paragraph editing.
**Architecture:** a stable host owns the viewport; a session-bound context carries the dialog's canvas presentation. Preview/document ownership stays in existing transactions.
**Tech Stack:** React, TypeScript, Vitest, Tauri/WebView2 and the existing Windows harnesses.
**Spec:** docs/superpowers/specs/2026-10-29-paragraph-viewport-lifetime-design.md

## Global constraints

Preserve the inherited tree, source raster cache change, failed evidence and old packages. No native/renderer/dependency changes. Keep 128 MiB process-tree growth and all original fidelity assertions. No forced GC, blocked diagnostics, commits or publication. One Cargo/owned GUI gate at a time. Exact process ownership and immutable inputs during verification.

## Review focus

Session/page ownership, stale cleanup, missing presentation before layout effects,
render feedback loops, and source/candidate lifetimes during Apply/Cancel/tab change.

## Task 1: stable viewport and presentation ownership

Files: src/components/ParagraphViewportHost.tsx (new), src/components/ParagraphWorkspace.tsx,
src/components/paragraph-workspace.css, src/App.tsx,
tests/occurrence-paragraph-app.test.tsx and tests/paragraph-viewport-host.test.tsx (new).

- [x] Snapshot affected inputs and observe the App viewport-identity regression fail.
- [x] Implement the host, matching publication/cleanup and loading fallback.
- [x] Render paragraph controls beside its stable viewport; preserve standalone mode.
- [x] Verify DOM identity through Preview/Cancel/Apply/Undo/Redo, stale sessions and cleanup.
- [x] Run focused tests, all frontend tests and production build; inspect terminal results.
      Focused 34 passes; corrected full run 1,326 passes/200 files, production build and integrity checks exit 0. First test-only TypeScript failure retained.

## Task 2: actual optimized runtime and fidelity

- [x] Bind and build a fresh optimized candidate without overwriting prior outputs.
      Optimized GUI and source check pass; unchanged CLI reused with explicit input proof.
- [x] Run original Word and mixed resource workloads; inspect memory, exact Cancel and source closure.
      Fresh build-2: both complete 23 functional cycles and source closure; both fail the original process-tree growth budget. An unchanged Word repeat also fails.
- [x] Run narrowed paragraph save/reopen and independent text/pixel/font/link/tag audit.
      Fresh build-2 narrow-2 passes, including 171 unselected glyphs and 43 structure objects.
- [x] Record the demonstrated result and any remaining failure. Continue diagnosis if budgets fail; do not relabel it success.
      runtime-reconciliation-2.json preserves original failures, 47 stable viewport/SVG/image identity observations, and diagnostic-only paint controls. Resource acceptance remains open.
- [ ] Complete portable integration and the one final scoped review after acceptance.

## Evidence-driven correction before Task 2 acceptance

Original optimized Word/mixed functional checks pass; resource budgets fail. Narrow independent audit passes. Real DOM observation proves viewport/SVG identity but reveals image replacement while loading. Additional files: src/components/PageView.tsx and tests/page-raster-identity.test.tsx.

- [x] Observe a regression failing when PageView replaces its raster image during pending preview.
- [x] Keep one keyed raster image mounted; absent URL remains blank with the existing loading/error state.
- [x] Verify loading/error/source restoration, canvas, rotation and crop behavior; run full frontend/build.
      39 focused passes; frontend-3 passes 1,327 tests/201 files, production build and integrity checks.
- [x] Build a fresh candidate in build-2 and rerun original runtime/fidelity gates plus passive identity observation.
      Optimized GUI/source checks pass; all element identities persist. This does not resolve the memory failure. No further speculative product correction: isolate the remaining page-paint contribution first.
