# Active document tab visibility

> Use superpowers:executing-plans inline. User authorized continuous work without
> routine approval gates. Previous cycle was verified progress.

**Goal/spec:** Opening, restoring or switching documents, closing tabs and resizing
the window should reveal the active tab's label and close button in the horizontal
strip. Scroll only that strip by the necessary distance. Do not focus the tab,
move the page viewport, change per-document state, or continuously fight manual
scrolling through inactive tabs. If the strip is narrower than a tab, align its
start predictably. Retain existing keyboard selection and document lifecycle.

**Evidence:** `artifacts/form-affine-20261023/desktop/sheared-font-reedited.png`
shows the last selected tab mostly outside the strip after resizing ten tabs.
App.tsx has no active-tab visibility effect; CSS only provides overflow scrolling.
Browser reproduction will use the existing small tab-performance fixture. T3's
preview was attempted; two snapshot calls failed with
`Preview automation snapshot timed out after 15000ms.` Use the repo Playwright
workflow fallback and actual isolated Windows WebView for acceptance.

**Approach:** A focused React hook observes the strip and active wrapper sizes,
and reveals it on active ID / tab-count changes. Read actual rectangles and change
only strip.scrollLeft, bounded by scrollWidth-clientWidth. A ResizeObserver handles
layout changes; window resize is the fallback. No scroll listener. Prefer this to
scrollIntoView, which can also scroll ancestors, or fixed-width index arithmetic,
which does not follow the existing responsive flex layout. No public API changes.

**Files:** `src/editor/use-active-tab-visibility.ts`, `src/App.tsx`;
`tests/active-tab-visibility.test.tsx`, `tests/active-tab-visibility.browser.mjs`;
Windows acceptance script and checkpoint artifacts.

- [x] Reproduce open/resize clipping in a real browser and save failing evidence.
- [x] Add meaningful geometry/focus/manual-scroll/lifecycle tests. Implement hook
  and attach the strip ref. Observe active wrapper and strip; clean observers and
  listeners on change/unmount, including empty-workspace transitions.
- [x] Verify narrow/wide resizing, programmatic/keyboard first/last switching,
  closing an inactive tab, opening a new one, and manual scroll staying put on
  ordinary edits. Confirm focus and PDF viewport scroll remain stable on resize.
- [x] Run focused and full frontend tests, typecheck/Vite/debug build; one scoped
  review; actual Windows multi-PDF workflow with undo/history and focus checks.
- [x] Record source/exe hashes, screenshots, cleanup and preserved prior artifacts;
  update quality ledger. Native code is unchanged, so retain the prior 501-test
  native result explicitly rather than rerunning the full suite.

Review focus: browser scroll anchoring during flex resize; all hooks before empty
workspace return; oversized tabs and fractional geometry; stale observer callbacks;
focus/document scroll and deliberate manual strip scrolling.

Verified checkpoint: `artifacts/tab-visibility-20261023/verification.json`.
752 frontend tests / 107 files, nine browser flows, seven Windows workflows.
Native source/dependencies unchanged; prior 501-test result explicitly retained.
No confirmed review defects. Active/last close and oversized strip browser checks
were added in response to coverage notes. Focus check stays above the existing
850px properties-hiding breakpoint; that separate usability gap is next.
