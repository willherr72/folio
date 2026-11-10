# Nested form text-state diagnosis implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline.

**Goal:** Reproduce the renderer discrepancy independently and pin Folio's safe behavior.

**Architecture:** An explicit-DLL Python probe compares synthetic PDF pairs; native
integration tests verify Folio's existing preservation boundary. No renderer swap.

**Tech Stack:** Python ctypes, pypdf, PyMuPDF, Pillow, Rust/lopdf/PDFium.

**Spec:** `docs/superpowers/specs/2026-10-24-form-inherited-state-design.md`

## Global constraints

- Preserve source PDFs, the dirty worktree, previous evidence and current DLL.
- Keep all appearance and glyph preservation checks intact.
- No commits, external publication or parity claim.

## Review focus

- Incorrect FFI declarations or object lifetimes invalidating native evidence.
- Non-equivalent reference documents invalidating the comparison.
- Known-defect mode accidentally treating newly broken controls as success.
- Local overrides unnecessarily disabling valid editing.
- Missing evidence for renderer replacement versus source-only inference.

## Task 1: Direct reproduction and Folio regression boundary

Files: `scripts/probe-pdfium-form-inheritance.py`,
`src-tauri/tests/form_text_state.rs`, and diagnostic artifacts.

- [x] Create and run a direct-DLL probe with individual/control/combined pairs.
- [x] Run normal mode and confirm failure; diagnostic mode must pass only the known matrix.
- [x] Add native inherited rise/scale refusal and supported override/control cases.
- [x] Run the native state/flow integration suites; record exits and counts.
- [x] Review the scoped probe, tests, conclusions and remediation requirements once.
- [x] Record evidence, runtime/toolchain limits and the pending renderer correction.

The integration tests pin existing behavior; no invented RED/GREEN product fix is
claimed. The direct probe's normal-mode failure is the pending dependency defect.
