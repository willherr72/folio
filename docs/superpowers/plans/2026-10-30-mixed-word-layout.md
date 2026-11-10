# Mixed-format Word layout acceptance plan

> **For agentic workers:** Use superpowers:executing-plans for inline execution.

**Goal:** Verify and, where demonstrated necessary, correct mixed/highlighted real
Word paragraph editing through layout changes and saved re-editing.

**Architecture:** Author two immutable Word exports and use the accepted extracted
0.26 CLI/desktop. Bind each source, request, binary and output; independent readers
judge fidelity. Implementation follows a demonstrated defect rather than a guess.

**Tech Stack:** Existing Python DOCX/Word COM, MuPDF/pypdf, packaged Rust CLI,
Playwright with owned WebView2 and native file dialogs.

**Spec:** `docs/superpowers/specs/2026-10-30-mixed-word-layout-design.md`

## Global constraints

- Preserve original exports, old artifacts/packages and unrelated dirty work.
- Only newly owned Word/Folio processes; no private quiz input or external navigation.
- No new dependencies, Cargo, builds or product edits until a defect is reproduced.
- One native/desktop workload at a time. Keep source/request-defined pixel masks.
- Standing user autonomy supersedes routine approval stops. Retain evidence and do
  not make commits or delete workspaces in this inherited dirty worktree.

## Review focus

- A highlight crossing a newly wrapped line must retain its text ownership (task 3).
- Source-only italic/bold glyphs must not silently receive a substitute font (tasks 2/3).
- Too-small edits must preserve the styled draft and unchanged document (task 2).
- Saved replay must not shift later baselines or change extraction order (task 3).
- Tagged and untagged exports must both retain their existing surrounding graphs (task 3).

## Task 1: Authored producer fixture and native discovery

Files: `artifacts/mixed-word-layout-20261030/generate.py`, `probe.py`, `progress.md`.
Consumes the package binding in `artifacts/portable-026-20261030/extracted-package.json`.
Produces `inputs-1/manifest.json`, immutable tagged/untagged source PDFs, and a
source-bound native probe report containing actual selections, requests and refusals.

- [x] Generate the DOCX and two Word exports with the exact spec ingredients.
- [x] Verify both originals with MuPDF/pypdf; require two pages, styles/highlight,
  links, embedded fonts, and nonempty tagged structure. Verify owned Word exit.
- [x] Inspect text/paragraphs and attempt highlighted insertion plus narrowing
  with the packaged CLI. Retain every result; identify accepted geometry and an
  insufficient-area refusal, or reproduce a product defect without changing source.

## Task 2: Existing desktop workflow

Files: `artifacts/mixed-word-layout-20261030/desktop.mjs`, `run.ps1`.
Consumes task 1's source/request report. Produces per-export workflow receipts,
screenshots, native request/close evidence and saved/replayed PDFs with hashes.

- [x] Use the captured owned-process launcher/dialog helpers and fixed native size.
- [x] Select only the target highlighted phrase; insert text with keyboard input.
- [x] Exercise canvas resize, insufficient-area refusal, visible Preview/Cancel,
  Apply and exact source/edited Undo/Redo. Assert the full styled draft survives refusal.
- [x] Save, reopen, inspect the preserved styles/geometry and preview unchanged text.
  Stop owned apps and retain originals. A failure is data, not a passed gate.

## Task 3: Independent output and completion audit

Files: `artifacts/mixed-word-layout-20261030/audit.py`, `controls.py`, `checkpoint.json`.
Consumes immutable originals, exact requests and saved/replay outputs from task 2.

- [x] Audit both readers' complete expected text/order, every selected style,
  baselines, highlight geometry, fonts, outside glyphs/pixels and untouched pages.
- [x] Compare links/bookmarks and nonempty tagged structure using the established
  graph audit, retaining coverage limits instead of declaring empty traversal proof.
- [x] Run negative controls for style/color, later baseline, expanded highlight
  and outside ink; each must fail its own check.
- [x] Run one fresh scoped final review of frozen artifacts and any product delta.
  Correct material findings locally with regressions; update the quality ledger.

Execution record: inline, dependency-ordered acceptance work. Product fixes, if
needed, get a concrete scoped plan amendment and focused regression gate first.
