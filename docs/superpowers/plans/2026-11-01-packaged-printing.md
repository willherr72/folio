# Packaged Windows printing implementation plan

> Execute inline with superpowers:executing-plans; one final fresh scoped review. Existing isolated worktree and standing autonomous authorization apply.

**Goal:** Verify and improve actual everyday printing from the accepted portable editor.
**Architecture:** Bind the existing package, drive the real app and owned Windows dialogs, audit driver output independently, and correct demonstrated defects without weakening fidelity requirements.
**Tech stack:** Tauri/Rust product, Playwright/CDP, PowerShell/Win32, Python PDF/image readers.
**Spec:** docs/superpowers/specs/2026-11-01-packaged-printing-design.md

## Global constraints
Only Microsoft Print to PDF; unique owned destinations; process birth/path/hash and dialog identity before actions; no global printer changes or physical jobs. Preserve all inputs/evidence. Package0.28 is the baseline, not a new build claim.

## Review focus
Wrong printer or stale process/dialog; cancellation that still submits; printed state diverging from unsaved edits; scale/crop/rotation/order mistakes; weak pixel/semantic oracles or unbound output provenance.

### Task1: Bind and observe the real native path
- [x] Reverify accepted0.28 checkpoint/package and capture current printer/source/helper identities.
- [x] Create owned fixture/probe and inspect the real Print dialog; verify cancellation and unchanged session.
- [x] Select the exact virtual printer and fresh destination; inspect one complete output and establish the full-content/physical-placement oracle before broader acceptance.
### Task2: Complete packaged print workflows
- [x] Add guard/oracle negative controls and observe RED→GREEN for new guarded behavior.
- [x] Run cancellation, invalid range and five print-mode cases with unsaved text/form/annotation edits and physical-frame coverage.
- [x] Audit page selection, complete content, placement, visibility, input/session preservation and cleanup. Diagnose/fix any demonstrated product defect with appropriate regression/build checks.
### Task3: Review and checkpoint
- [x] One final scoped review; correct consequential findings and preserve failed attempts.
- [x] Verify exact accepted evidence, update quality roadmap/continuation, leave overarching goal active.
