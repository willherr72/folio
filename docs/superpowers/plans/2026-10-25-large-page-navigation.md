# Bounded Thumbnail Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans, inline. One final scoped correctness reviewer.

**Goal:** Remove measured large-document thumbnail UI stalls while preserving logical navigation, selection, dragging and PDF output.

**Architecture:** Pure fixed-row geometry feeds a shared ResizeObserver/scroll hook. Sidebar and organizer retain full page data but mount nearby rows plus focused/drag-source rows. Native rendering/cache and document transactions stay unchanged.

**Tech Stack:** React/TypeScript, existing CSS, Vitest, packaged WebView2 workflow.

**Spec:** docs/superpowers/specs/2026-10-25-large-page-navigation-design.md

## Global constraints

- Two overscan rows each side, plus focus/drag source rows; never pin all selected pages.
- Sidebar 140px rows + 4px gap, organizer 186px/135px minimum/12px gap; narrow 152px/105px/6px.
- Full-model keyboard/ranges/order; no source registration/export changes.
- No hidden-host render-all fallback, new dependencies, commits/releases or original overwrites.
- Preserve prior checkpoint, baseline partial failure and exact same-process 10,000 continuation.

## Review focus

- Target outside rendered range must mount before focus, including End and restored selection (task 2).
- Manual scrolling must retain focus/drag without jumping back (task 1/2).
- Width changes preserve logical context; last partial row and deletion clamp correctly (task 1).
- Sidebar global navigation/permissions and parent drag behavior survive extraction (task 2).
- Extraction freeze, source ownership and saved page identity remain unchanged (task 2/3).

### Task 1: Geometry and measured window hook

Create src/editor/page-window.ts, src/components/usePageWindow.ts, tests/page-window.test.ts.
PageWindowMetrics {columns,itemWidth,itemHeight,gap,paddingTop,height}; pageWindow(count,metrics,scrollTop,retained,overscan=2) returns {indices,totalHeight}; revealPage(index,metrics,scrollTop) returns aligned visible scrollTop; anchorPageWindow(scrollTop,before,after) returns context-preserving top.
Hook usePageWindow(hostRef,{count,initialIndex,retained,singleColumn?}) returns metrics,indices,totalHeight,style(index),reveal(index). CSS custom properties define geometry. Hook owns resize/scroll observers and cleanup; no native resources.

- [x] Write red pure geometry tests for bounded visible ranges, distant retained rows, duplicate/invalid retention, partial last row, hidden geometry, reveal and responsive anchor.
- [x] Implement pure model and hook with finite clamped geometry, sorted indexes, animation-frame coalescing and resize anchoring.
- [x] Verify focused geometry tests and TypeScript.

### Task 2: Organizer and sidebar integration

Modify PageOrganizerDialog.tsx, page-organizer.css, App.tsx, styles.css. Create PageThumbnails.tsx. Add tests/page-window-ui.test.tsx and tests/page-thumbnails.test.tsx, retain organizer/App tests.
PageThumbnails consumes existing adapter/pages/current ID/blocked/canArrange/drag state and parent drag callbacks. Emits onSelect(pageId); keyboard activates logical targets. Its hook state remounts per tab.

- [x] Add failing large-window tests for bounded tiles and far keyboard/range/Undo focus; mounted focused/drag source through scrolling; resize columns and last row.
- [x] Replace organizer all-page map with window indexes/styles, full-model columns and pre-focus reveal; add full-list ARIA position/size; memoize dirty digest.
- [x] Extract sidebar rendering, preserve class/labels and drag handlers, add logical roving keyboard navigation and selection reveal; retain focused/drag rows.
- [x] Run focused existing selection/drag/navigation/protection/extraction tests and TypeScript; resolve failures with exact regressions.

### Task 3: Verification and checkpoint

- [x] Freeze scoped hashes; one final correctness review and one verified correction pass.
- [x] Full bounded frontend tests and fresh fingerprinted build; native code unchanged.
- [x] Repeat identical 100/1,000/10,000 desktop measurements, record raw samples/mounted counts/memory and compare baseline with caveats.
- [x] Verify far keyboard/Shift selection, scroll/resize, real drag/autoscroll, narrow controls and Apply/Undo; independently verify reordered/rotated/extracted saved pages and originals.
- [x] Visually inspect screenshots, verify loaded PDFium, stop owned processes, update quality ledger/README and checkpoint hashes. Keep broad goal active.

Evidence: artifacts/large-page-navigation-20261025/checkpoint-final/verification.json. Full suite before final grid-level edge-scroll correction; final focused/TypeScript/build/desktop after it. Native exports are correct but slow at 10,000 pages, explicitly retained as the next profiling target.
