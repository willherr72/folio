# Paragraph source raster reuse implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline. Preserve the
> inherited tree. Include this correction in the one final scoped milestone review.

**Goal:** avoid rerendering an unchanged unannotated source page on every paragraph session.
**Architecture:** reuse PageView's existing bounded image cache and ownership;
keep annotated originals and candidate previews on the serialized local path.
**Tech Stack:** React/TypeScript, Vitest, Tauri/WebView2, existing Windows harnesses.
**Spec:** docs/superpowers/specs/2026-10-29-paragraph-source-raster-reuse-design.md

## Global constraints

Preserve the 0.25 package and all failed evidence. No native-engine, renderer,
dependency or PDF-content changes. Retain 60-entry/96 MiB cache limits, 128 MiB
tree-growth and all other runtime budgets. No forced GC or blocked diagnostic
actions. No commits, publication or user-app restarts. Only one owned GUI/Cargo
gate at a time; freeze inputs during gates.

## Review focus

1. Reopened/parallel views share source rasters without premature revocation.
2. Closing a source releases pending and completed rasters exactly once.
3. Annotated pages and new candidates retain their distinct composition/ownership.
4. Failed rendering retries and source/page/zoom changes use correct images.
5. Fewer native requests must not be misreported as a proven memory improvement.

## Task 1: bounded source reuse with ownership regressions

Files: src/components/PageView.tsx, src/components/ParagraphWorkspace.tsx,
tests/paragraph-workspace.test.tsx, tests/paragraph-workspace-cache.test.tsx.

- [x] Capture baseline source/test inputs and failed runtime evidence.
- [x] Add meaningful reuse/ownership/retry/composition regressions; observe expected failures.
- [x] Export the existing image hook with retry revision and minimal page type.
- [x] Use it for unannotated original pages; preserve local composed/candidate renders.
- [x] Run focused tests, full frontend suite and production build; inspect terminal results.
      Focused 19 passes; full frontend 1,322 passes; production build and integrity checks exit 0.

## Task 2: measure the actual corrected candidate

- [x] Build a fresh source-bound local candidate without replacing the failed package.
      Optimized GUI build and source check exit 0; CLI is explicitly reused from verified package.
- [x] Run the original full Word resource workflow with unchanged budgets/assertions;
      compare original-page render counts and per-process memory.
- [x] If memory remains outside budget, preserve the result and continue diagnosis;
      do not claim this correction resolves that separate requirement.
- [ ] Verify mixed/narrow saved output and source ownership, then integrate with the
      remaining portable milestone gates and one final review.

## Runtime checkpoint

Optimized source `04161d48a914ef195527f382741ba08465f5451d2425b3253ce8b90dfe8670e3`: Word and mixed each completed 23 functional cycles with exact Cancel restoration and candidate closure. Both fail unchanged process-tree growth budget (244,195,328 and 200,962,048 bytes). The Word original page now renders once instead of 24 times. No memory-resolution claim.

Narrow paragraph refusal, Apply, Undo/Redo and save/reopen pass. Independent audit passes: 171 unselected glyphs, surrounding pixels, second page, links, fonts and 43 tagged-structure objects preserved. All owned apps stopped; source fingerprint remains unchanged.

Opening the paragraph workspace alone still grows 233,951,232 bytes. Hidden-pane diagnostic reduces this to 118,964,224; removing page shadows or the tiled background does not resolve growth. Next hypothesis: viewport teardown/recreation at App.tsx paragraph workspace selection. Controls are diagnostic only. Full evidence: `artifacts/paragraph-source-raster-reuse-20261029/runtime-comparison-2.json`. Remaining portable/review gates stay open.
