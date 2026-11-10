# Stable paragraph baseline anchors implementation plan

> Use superpowers:executing-plans inline and test-driven development. User's
> standing authorization replaces routine approval pauses. One final reviewer.

**Goal:** retain the first baseline through changed glyph heights, fitting and
saved re-edit, while explicit movement continues to work.
**Architecture:** optional displayed physical-point anchor crosses the current
API; worker layout uses its raw baseline Y; version7 replay proves saved intent.
**Tech stack:** current Rust/PDFium/lopdf, TypeScript/React, CLI/MCP and WebView2.
**Spec:** docs/superpowers/specs/2026-10-28-paragraph-baseline-anchor-design.md

## Constraints and review focus

Preserve source proofs, exact clips, existing font/ink policy and finite search.
One Cargo, locked/offline/jobs1/serial tests; no source/test edits during gates.
No commits, release/package overwrite or user-app restart. Freeze baseline first.
Review/test: physical point roundtrips; leading blank lines; forged old-version
anchor metadata; Fit versus Move versus Resize; baseline near clips/neighbor ink.

## Task 1: native layout, fitting and saved intent

Files: `types.rs`, `physical_units.rs`, `form_paragraph.rs`, `text_paragraph.rs`,
`styled_paragraph_plan.rs`, `styled_paragraph_layout.rs`,
`paragraph_highlight_geometry.rs`, `styled_paragraph_area.rs`,
`styled_paragraph_memory.rs`; tests `styled_paragraph_baseline_anchor.rs` and
focused layout/memory/physical tests. Existing request literals receive explicit
None to preserve their old test contracts; new behavior tests opt into the anchor.

- [x] Freeze source baseline and create a current native/CLI reproduction: Courier
  baseline600, `AA BB` -> `AA BB CC`; assert no move and observe semantic failure.
- [x] Add optional Point to selection/request; use the existing public/raw scaling
  and geometry point conversions. Restore exact unchanged source point components
  on physical roundtrip in top-level/occurrence/form facades.
- [x] Add optional first-baseline argument through measured layout, preserving
  existing wrappers for legacy callers. Validate top/bottom reservation; test
  higher/lower glyph ascent, translation, blank first rows and invalid anchors.
- [x] Fit expands top to measured first ascent above the fixed baseline. Keep
  exact source-context/page/obstacle checks on every anchored trial. Test clear
  and blocked headroom across rotations and UserUnit1.7.
- [x] Add version7 optional first_baseline memory with strict version validation,
  exact fragment replay and retained raster proof; expose the verified first
  logical baseline on inspection. Test saved two-edit/no-op replay and forgery.
- [x] Run focused units/integrations; independently inspect saved outputs in
  MuPDF/pypdf and compare unchanged neighbors, actual baselines and source bytes.

## Task 2: desktop and automation

Files: `src/editor/types.ts`, `styled-paragraph-editor.ts`,
`src/components/StyledParagraphDialog.tsx`, `src-tauri/src/automation/paragraph_preview.rs`;
associated editor/dialog/automation tests and any request fixtures that assert
the complete protocol shape.

- [x] Add anchor to immutable snapshots/requests. Test text/style retention,
  position translation, resize retention, explicit fit retention, reset and undo.
- [x] Dialog tags Fit as fit intent; ordinary area changes distinguish position
  from size changes. Preserve anchor through preview/cancel and saved reopen.
- [x] CLI/MCP request builder carries source anchor and translates for explicitly
  supplied bounds before Fit. Shared nested/occurrence routing retains it.
- [x] Run focused frontend and native automation tests; broaden to affected
  paragraph/form/context regressions, complete frontend tests and production build.
- [x] Freeze final scope and dispatch one review; address demonstrated findings
  in one correction pass. Build current GUI/CLI with source fingerprint.
- [x] Exercise changed-ascent CLI and Windows editing, fitted preview Cancel,
  explicit movement, Undo/Redo, saved re-edit; audit text order/glyph baselines,
  surrounding pixels/fonts/links and nonempty tagged graphs where present.
- [x] Write checkpoint, dispositions and roadmap; retain failures/limits and keep
  the full quality goal active.


Completed local source milestone; authoritative evidence and limits: artifacts/paragraph-baseline-anchor-20261028/checkpoint-final-2.json. One final review and one correction pass. Broad native evidence precedes corrections; final source has complete library and eight affected integration consumers plus full frontend, fresh build, desktop and MCP acceptance. Independent audits are synthetic single-font cases; no new tagged-source, printing, low-memory or commercial-parity claim.
