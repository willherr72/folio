# Bookmarks implementation

Spec: ../specs/2026-10-22-bookmarks-design.md
Keep the existing isolated worktree and prior authorized changes. Implement inline
with TDD and one fresh read-only review of the milestone after integration.

- [x] Task 1: Native portable outline model, bounded import and atomic export.
  Add missing-API integration tests first, including hierarchy/styles/named targets,
  action preservation, reordered/extracted pages and malformed graphs. Implement
  engine read/cache and export plumbing. Expected: RED missing API, then native
  roundtrip tests and existing export/recovery suites pass.
- [x] Task 2: Frontend history, page remapping, recovery and adapter plumbing.
  Add failing model/recovery tests, implement transforms, preserve explicit empty
  trees, and cover append/delete/extract/text source replacement. Expected: exact
  bookmark targets and history survive all document operations.
- [x] Task 3: Bookmark sidebar and page navigation. Add failing App tests, implement
  accessible nested rows and edit controls, then verify keyboard/focus and Find
  races. Expected: full workflow, full frontend suite and production build pass.
- [x] Task 4: Native desktop/independent-reader acceptance and fresh review. Fix
  material issues RED→GREEN, record deferred findings and update parity evidence.
  Expected: saved/reopened/recovered bookmarks match actual edited page order.

Review focus: recursive PDF graphs and portable stream limits, named destinations
inside action chains, stale page references, extraction of ancestors, explicit
empty vs absent tree, source replacement and recovery without original files,
duplicate title/IDs, focus after moving/removing rows, small-window controls.

Completed: 385 frontend tests, production and Windows debug builds; 10 bookmark,
7 export, 14 recovery and 9 printing tests pass (two physical-printer tests remain
ignored). Fresh review's three Important findings were reproduced and fixed:
malformed roots hidden by PDFium, focus falling to a hidden child after deletion,
and indirect chains bypassing the 32-level budget. One fix pass; no re-review.

Real desktop acceptance: import/edit/move bookmarks, reorder pages, undo/redo,
save, extract, forced-exit recovery, save/reopen, page jump, and a 900×600 dialog.
MuPDF and pypdf agree on hierarchy, styles and targets. Raster pixels and original
source hash are unchanged. Evidence: artifacts/bookmark-smoke/verification.json,
artifacts/desktop-bookmarks-*.png and artifacts/bookmarks-progress.md.

Rulings: retain uncommitted local checkpoints for the ongoing project loop;
preserve external actions without executing them; nested creation and sibling
movement are supported, while moving an existing branch to another parent is a
future enhancement. No Adobe/Foxit parity claim.
