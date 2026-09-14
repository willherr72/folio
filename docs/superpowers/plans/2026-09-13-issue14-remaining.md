# Issue 14 remaining acceptance plan

> Execute with superpowers:subagent-driven-development; preserve passing v0.10.0 behavior.

**Goal:** Resolve remaining issue #14 gates where supported by exact reader and editing evidence; retain explicit refusals for any representation that fails.
**Spec:** Existing docs/superpowers/specs/2026-09-11-shaped-text-design.md and GitHub #14 acceptance criteria.

The user authorized finishing #14. Continue in the existing shaped-text worktree. No blanket feature widening or speculative removal of guards. Current native outlines remain the visible truth. Investigate a single wide CID semantic font to avoid font switches; test mixed-direction and RTL separators separately because reader bidi is an independent constraint. Whole-line ActualText alone is already disproven for selection geometry.

- [x] Test a single wide semantic font against the retained banked PDFs using PDFium, MuPDF, pypdf and original cluster geometry at all rotations. If it passes, implement a bounded native writer with regressions before enabling it. If it fails, preserve exact negative evidence.
- [x] Implement grapheme-safe plain Backspace/Delete in shaped Content editing, with original Unicode preserved, one undo, correct selection and no interference with IME/native shortcut behavior. Test combining sequences, supplementary Unicode, ligature interiors and RTL boundaries.
- [x] Probe remaining bidi/RTL reader cases with exact Unicode and geometry; widen only passing cases.
- [x] Complete available real-reader/input acceptance without claiming hidden automation as physical manual testing. Record any external acceptance blocker honestly.
- [x] Run relevant native/frontend/browser checks; independent review; update issue acceptance status with evidence. Close #14 only if its acceptance is satisfied or explicitly agreed scoped.

Resource rules: native cargo uses shared primary debug target and -j1, one cargo process at a time. Python uses -B -X utf8. Test synthetic/public files only. No desktop process termination without exact owned PID/path checks.

Results: 143 native tests, 276 frontend tests, 11 Chromium editing cases, and 48 native PDF original/resaved reader/model checks pass. Mixed-direction metadata policies fail; guards remain. Manual Foxit/physical IME acceptance is pending; #14 remains open. Packaged Windows acceptance passed, including native dialogs, wide text, grapheme deletion, copy/search, Arabic/Indic exports and crash recovery. v0.10.1 packaging follows; the remaining manual and bidi gates stay open.
