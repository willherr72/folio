# Toolbar visibility implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline, with one fresh
> focused review after implementation. Continue under existing user authorization.

**Goal:** Keep document actions visible and editing tools readable at supported sizes.
**Architecture:** Reorganize existing App toolbar groups into two logical rows;
CSS supplies natural height and button wrapping. Keep command handlers unchanged.
**Tech Stack:** React/TypeScript, CSS, existing Vite and browser acceptance tools.
**Spec:** `../specs/2026-10-23-toolbar-layout-design.md`.

## Global constraints

Supported checks900x600 and1280x820 CSS pixels. Preserve ready portable0.17, native
code, all prior uncommitted work and original documents. No version bump or publish.

## Review focus

- Protected reader adds Unlock editing: measure all buttons with it present.
- Save options portal must stay anchored and keyboard dismissible after reflow.
- Browser zoom or narrow width must wrap whole controls without clipping labels.
- Disabled actions and command shortcuts must retain their previous behavior.
- Extra toolbar height must leave a usable scrollable PDF viewport at600px height.

## Task1: Reproduce and implement layout

Files: `src/App.tsx`, `src/styles.css`, `src/components/review.css`,
`tests/fixtures/toolbar.html`, `tests/fixtures/toolbar.tsx`.

- [x] Create real-App synthetic native-adapter fixture, selectable reader/theme
  via URL parameters, and inspect current overflow with the collaborative browser.
- [x] Reorganize JSX into named Document actions and Editing tools groups. Preserve
  existing button props and handlers, moving utilities beside creation/open actions.
- [x] Replace fixed toolbar height with auto; make primary row stable and editing
  buttons wrap without internal wrapping/shrinking. Remove old overflow override.
- [x] Measure button rectangles, overflow and usable workspace at required sizes.

## Task2: Verify and review

Files: browser evidence under `artifacts/toolbar-layout`, quality ledger and plan.

- [x] Run existing frontend tests and production build; inspect terminal results.
- [x] Exercise actual keyboard Tab/Enter/Escape through toolbar and Save menu,
  verify protected-reader disabled controls, dark theme and browser zoom/reflow.
- [x] One fresh read-only review of explicit changed scope; reproduce/fix material
  findings in one pass. No per-task implementation delegation.
- [x] Record evidence and remaining limits in docs/product-quality-loop.md; mark
  this milestone complete only when layout, actions and build checks pass.

## Completion evidence

Frontend588/91files passes; final production build exit0. Browser measurements and
screenshots, full toolbar Tab traversal, Save menu Enter/Escape/anchor, Find/Ctrl+F,
zoom and protected controls pass in artifacts/toolbar-layout. Five layouts include
853x547 as150%-zoom-equivalent reflow, not actual WebView/OS scaling. Fresh review
has no remaining findings; original stale overflow rule removed and rechecked.
No native changes or new portable package. Full findings/rulings in the ledger.
