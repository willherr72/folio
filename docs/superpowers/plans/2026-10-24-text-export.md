# Text Export Implementation Plan

> Use superpowers:executing-plans inline, TDD, and a final scoped review.

**Goal:** Export current edited pages to bounded UTF-8 text without mutating PDFs.
**Architecture:** Native per-page composition/extraction with resource leases and
window-owned cancellable publication; frozen frontend range dialog in Save menu.
**Tech Stack:** Existing Rust/PDFium/lopdf, Tauri, React/TypeScript and Windows CDP.
**Spec:** docs/superpowers/specs/2026-10-24-text-export-design.md.

## Constraints and review focus

Preserve dirty worktree and source files; no commits/releases. Native copy rights
authorize export. Do not export password values, mutate source geometry, leak
temporary sources/resources, silently truncate text or overwrite on cancellation.
Crop inclusion is geometric selectable text, not a visibility/redaction guarantee.

- [x] Native contract/extractor: add text_export.rs and engine text export module;
  meaningful failing integration tests in tests/text_export.rs. Add TextExport
  intent and bounded reader/composition path with OCR ordering and field sections.
- [x] Capture resources, validate destination, implement cancellation/atomic
  publication and job commands; exercise limits, permissions and cleanup.
- [x] Add editor/text-export.ts, ExportTextDialog.tsx, SaveCopyButton/App wiring;
  failing workflow/selection/job tests, then implementation and focused checks.
- [x] Run relevant native and full frontend tests; freeze scope, independent review,
  fix reproduced findings and stage a fresh source-fingerprinted executable.
- [x] Run Windows acceptance and independent text/source checks; stop only owned
  test process, record checkpoint and update quality ledger without parity claim.

Evidence root: artifacts/text-export-20261024. Native/build commands are serialized
with the shared Cargo target and one build job. Prior canvas-form checkpoint stays
intact. Next cycle may continue unfinished plan tasks; no partial release claim.

Completed scoped checkpoint: artifacts/text-export-20261024/checkpoint-final/verification.json.
