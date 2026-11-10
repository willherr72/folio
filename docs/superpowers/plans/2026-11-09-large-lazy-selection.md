# Large-document lazy-selection acceptance plan

> **For agentic workers:** Use superpowers:executing-plans inline. Reuse the existing reviewed runtime and direct coordinator verification. This is an acceptance extension without product changes; do not dispatch another reviewer for copied helpers.

**Goal:** Check current editing behavior and resource use on the generated 300-page PDF while preserving the entire 100-action history.

**Architecture:** Source-bound owned Windows/WebView2 session; existing native UI driver and independent PDF reader.

**Tech stack:** PowerShell, Node CDP and Python/PyMuPDF, with existing local dependencies only.

**Spec:** docs/superpowers/specs/2026-11-09-large-lazy-selection-design.md

## Global constraints

No product edits, accepted-package changes, new dependencies, commits or source deletion. Serialize compiler/test/desktop/resource work. Preserve fixture SHA476785062c3549b718de185653d5ad8ad8f3987fdae84ebcf65de15c4bd1c3f1. No style injection or forced GC. Do not present historical unmatched samples as performance comparisons.

## Review focus

- Exactly 300 page slots remain after open and save/reopen.
- All 100 Undo and 100 Redo entries retain the expected source text and paint.
- Edit mode has semantic current text and no unnecessary glyph geometry; Select/Highlight restore their geometry.
- Every saved page retains independently verified text, page boxes and paint.
- Failed resource targets and live/unknown process identities fail closed or remain explicitly open.

## Task: source-bound large-document acceptance

Files: create artifacts/large-lazy-selection-20261109/{prepare.py,run.ps1,session.mjs,audit.py}; copy unchanged native helper files with provenance.

- [x] Finish and seal the instruction-strip correction first; bind its checkpoint and optimized executable.
- [x] Prepare fixture, candidate and golden-output pins; copy the existing two-page native workflow with only the fixture/scope changes.
- [x] Run 105 edits, 100 Undo/100 Redo and save/reopen, preserving every intermediate result and resource sample.
- [x] Independently check all 300 saved pages against the sealed previously audited output; reject corrupted history and resource reports.
- [x] Reconcile process/profile absence, source stability and resource targets; record qualified behavior and any remaining performance gap. Keep the broad goal active.

Completed: the original owned desktop session passed all 105 edits, 100 Undo/100 Redo, current accessible text, Select geometry/copy, Highlight activation, save/reopen and exact state checks. PyMuPDF independently confirms all 300 pages' raw glyph data, boxes, rotation and full RGB at 72/144 dpi against the previously audited sealed golden output. Eight corrupted-history/resource controls pass. At edit 105, sampled native private bytes were 442,699,776 and whole-tree private bytes 1,028,177,920, meeting the predefined 512 MiB/1 GiB targets for that sample. All seven observed process identities and milestone WebView profiles are absent. No product source changed; no matched performance comparison, peak bound, broad producer or commercial-parity acceptance is claimed. Next qualify a combined local development portable for the visible UI improvements, retaining these limitations.
