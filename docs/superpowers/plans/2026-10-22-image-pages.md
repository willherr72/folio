# Image pages implementation

Spec: ../specs/2026-10-22-image-pages-design.md

Use the existing isolated worktree; implement inline and retain the uncommitted
project state. One fresh review after implementation. No push, release or PR.

- [x] Task 1: Native conversion. Add failing integration tests for PNG/JPEG pixels,
  orientation, fit, invalid input, export/recovery. Implement bounded decoder and
  PDF page construction, engine worker and public options. Run image_pages tests.
  Expected: missing API fails first; all conversion tests then pass.
- [x] Task 2: Desktop workflow. Add failing App/dialog tests for ordered files,
  options, new/insert undo, native cancel/errors and orphan cleanup. Implement
  picker/adapter and accessible modal, integrating source ownership. Run focused
  tests then all frontend tests/build. Expected: new controls initially missing;
  complete workflows pass without regressing existing ones.
- [x] Task 3: Acceptance and review. Build desktop, exercise generated PDF using
  independent reader and desktop/browser UI. Fresh reviewer checks bounds,
  ownership and atomicity. Fix material findings with regression tests. Update
  capability record and user-facing docs. Expected: verified complete workflow,
  remaining limits documented, no parity claim.

Review focus: EXIF flips as well as rotation, image decoder allocations before
dimension checks, mixed good/bad lists, pending picker dismissal/unmount, active
Find after insertion, recovery without image files, hostile paths and output size.
