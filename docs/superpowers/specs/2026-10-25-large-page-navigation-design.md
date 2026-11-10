# Large-document thumbnail navigation

## Intent and evidence

Improve everyday navigation and page organizing at the supported 10,000-page scale,
without changing PDF content, export or selection semantics. Standing user authorization
delegates routine design/implementation choices; no new permission checkpoint is needed.
The previous organizer milestone is verified progress, not evidence of overall parity.

Baseline on the verified organizer executable, 1400x950, same isolated Windows process:
100-page organizer opens in 27.5–47 ms; 1,000 pages in 170.7–373.3 ms; 10,000 in
1,883.2–2,104.6 ms. Select all at 10,000 costs 357–535.5 ms. All 10,000 option buttons
and 10,000 sidebar buttons are mounted. Sources: artifacts/large-page-navigation-20261025/
baseline/{measurements.json,measurements-10000.json}. Initial 10,000 open exceeded the
harness's five-second assertion; the same live process subsequently proved successful,
and the continuation measured it without restarting. Failed observation is retained.

## Approaches

1. Memoize every tile: lowers some React work but retains thousands of DOM nodes,
   observers and layout boxes. Insufficient for measured mount/layout cost.
2. Paginate: bounded DOM but changes continuous scrolling, range selection and dragging.
3. Render a window of fixed-height thumbnail rows, retaining focus/drag source rows:
   chosen coordinator approach. Preserves continuous scroll and selection while bounding
   mounted buttons. No dependency or raster/native change required.

## Design

Use a shared pure row-window geometry model and a React hook. Read tile height, gap,
minimum width and host padding from CSS custom properties/computed styles. Sidebar is
one column (140px tile, 4px gap); organizer uses responsive columns (135px minimum,
186px height, 12px gap; existing narrow breakpoint uses 105px/152px/6px).
The scroll host contains one relative spacer with full logical height. Render only
visible rows plus two overscan rows each side, absolutely positioning their buttons.
Also retain at most the current focus row and the drag-origin row; never retain all
selected pages. Sort rendered indexes in document order. No blank inaccessible spacer
or duplicate tile should enter the tab order or accessibility tree.

The hook measures host size through ResizeObserver (resize fallback), samples scrolling
with requestAnimationFrame, and clamps after count changes. Resizing preserves the
first visible logical page as the scroll anchor when column count/row stride changes.
No render-all fallback for hidden or zero-size hosts; use a modest initial viewport.
Reveal-index changes scrollTop and updates the render window before focus is requested.
Disconnect observers and cancel scheduled frames on unmount; no document registrations.

Organizer selection/history/extraction remains unchanged. Home/End and arrows operate
on the full page model using actual column count, not the mounted option count. Shift
ranges and Select all include unmounted pages. Focus/aria-posinset/aria-setsize describe
the full list. Initial current page, keyboard moves and restored Undo/Redo selection
reveal their destination. Manual scrolling must not snap back to a pinned focus row.
Drag source stays mounted even when far from the visible range, and group drag ownership
remains with the existing immutable page-array snapshot. Edge autoscroll continues;
drop targets resolve logical indexes rather than offsets in the rendered window.
Memoize dirty-digest calculation by document identity so scroll/selection does not
reserialize the complete draft; page actions still recompute and preserve no-op semantics.

Extract sidebar rendering into PageThumbnails with the existing thumbnail buttons,
labels, classes and parent drag callbacks. A single roving tab stop replaces thousands
of sequential Tab stops: ArrowUp/Down/Home/End focus and activate logical pages, while
Tab leaves the collection. Keep a focused page mounted during manual scrolling and a
drag source mounted through drag end. Reader/programmatic selected-page changes reveal
that page; manual sidebar scroll alone does not change selection or snap back. Remount
on active-tab change to avoid stale focus/scroll/drag references. Preserve parent drag,
protection, dirty/history and navigation logic. Small documents retain their layout.

## Scope and non-goals

No native changes, new dependencies, commits, source PDF overwrites, release or parity
claim. Do not change main-page continuous layout/text-range retention this cycle: it
already bounds expensive PageView instances, and thumbnail cost is directly measured.
The remaining all-page placeholders/geometry scans stay an explicit subsequent target
if fresh measurements show they dominate. Fixed thumbnail frames do not alter page size.

## Verification

Pure geometry tests: visible ranges, overscan bounds, last partial row, empty/hidden
containers, pinned distant focus/drag rows, scroll reveal and resize anchor.
Component/App tests: 10,000 pages mount a bounded number of tiles; End/Home and Shift
range use full data; selecting/rotating/deleting/Undo retains correct page identity and
focus; scroll keeps focused/drag source nodes; responsive column changes retain context;
sidebar keyboard/parent navigation/drag and pending organizer extraction remain valid.

Run relevant focused and full bounded frontend suites, TypeScript and fresh fingerprinted
Windows build. One final scoped correctness reviewer and one correction pass. Desktop:
same 100/1,000/10,000 fixtures and three samples, same viewport/machine/build profile,
without concurrent heavy work; record exact binary/source identity, mounted counts,
interaction timings, heap/DOM counters and process-tree snapshots with their limits.
Do not claim absolute peak memory or a universal latency guarantee. Require bounded
mounted tiles and substantial large-document improvement without small-document regression.
Verify far-page keyboard/Shift selection, scroll/resize, group drag/autoscroll, narrow
controls, Apply/Undo and saved reordered/rotated/extracted pages with independent reader.
Keep original hashes, failed runs and prior executables; stop only owned test processes.
