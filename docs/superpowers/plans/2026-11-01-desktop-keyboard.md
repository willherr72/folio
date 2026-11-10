# Native desktop keyboard editing plan

> For agentic workers: use superpowers:executing-plans inline.

**Goal:** Verify composition-safe shortcuts in a complete desktop PDF workflow.
**Architecture:** Frozen source debug candidate; owned app/profile; real frontend
and native services; independent fixed-mask saved-file audit.
**Tech stack:** Tauri/Rust, React, PowerShell, Playwright, PyMuPDF/pypdf.
**Spec:** docs/superpowers/specs/2026-11-01-desktop-keyboard-design.md

## Constraints and review focus
Preserve original fixture, user's sessions, portable0.29 and inherited work.
Only an owned process may be stopped, after binary/start identity verification.
Composition input is synthetic and must be labeled. Pin executable, DLL, inputs,
helpers, source and outputs. Test draft cancellation, composition shortcuts not
reaching document history, narrow action visibility, reopened metadata, text
neighbors and untouched page. Keep failed attempts separate and diagnose them.

## Task 1: Candidate
- [ ] Verify previous composition checkpoint pins and native/dependency equality.
- [ ] Capture current source; build frontend and debug custom-protocol desktop.
- [ ] Recheck source; copy executable and renderer into new candidate directory.

## Task 2: Desktop and audit
- [ ] Prepare captured helpers and fixture; launch owned instance at unused port.
- [ ] Run narrow existing-text Cancel/Apply, Undo/Redo, Unicode metadata and
  save/reopen/re-edit/search workflow, including flagged composition events.
- [ ] Stop owned app and verify input identities and owned process absence.
- [ ] Independently audit all saved states; run corruption controls.
- [ ] Fresh scoped review, fix demonstrated important defects and record checkpoint.

No commit, package overwrite or public release is part of this cycle.
