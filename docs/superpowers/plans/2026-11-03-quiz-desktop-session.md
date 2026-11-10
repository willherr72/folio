# Original quiz desktop session implementation plan

> Use superpowers:executing-plans inline in the existing isolated worktree.

**Goal:** Verify and correct demonstrated desktop failures in original-quiz paragraph editing.
**Architecture:** Owned WebView2 desktop/Windows dialogs, read-only IPC observation, immutable saved copies and independent Python audit.
**Tech stack:** Current Tauri/Rust/PDFium build, Playwright, PowerShell, MuPDF/pypdf.
**Spec:** docs/superpowers/specs/2026-11-03-quiz-desktop-session-design.md

## Constraints
Private originals/accepted package unchanged; no unrelated process termination. No direct native-edit bypass. Record failing attempts. Reuse native evidence only on matching source/test identities; product defects require RED regression and affected verification.

## Task1 â€” current desktop session
- [x] Build and pin current custom-protocol candidate; verify before/after source fingerprint.
- [x] Implement owned workflow and actual dialog control using existing helpers. Bind helper copies and process identity.
- [x] Exercise first insertion, multiline growth and highlighted styles; check Cancel, preview/apply, Undo/Redo, Save/reopen and second saved edit.

## Task2 â€” independent verification and checkpoint
- [x] Bind native request/run styles and saved copies; apply corrected original-source style/pixel oracle to every output.
- [x] Investigate failures and add focused RED tests before any product correction.
- [x] One fresh review; resolve Important findings once, record declined judgments and remaining coverage.
- [x] Verify originals/package/process cleanup and evidence; update roadmap. Goal remains active.

## Review focus
- UI harness must not bypass editing or fake native success; native requests are observations only.
- Contenteditable replacement must retain mixed styles; source-owned per-character audit checks this.
- Async previews/raster can be stale; capture only completed current preview and compare exact Undo/Redo.
- Saved output must be actual dialog-selected path, fresh file, bound by hash and independently reopened.
- Cleanup must target only PID/path/start-time identity; current viewport does not establish narrow-window or assistive-tech acceptance.
