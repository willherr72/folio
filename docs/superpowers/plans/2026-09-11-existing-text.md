# Existing PDF Text Editing Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development or superpowers:executing-plans to implement task by task.

**Goal:** Ship a tested, bounded first version of actual PDF text replacement for issue #8.

**Architecture:** Native text-object inspection and validated one-page derivation; frontend source-version commit using existing history, caches, export and recovery.

**Tech Stack:** Rust/PDFium, Tauri, TypeScript/React, Vitest, Playwright.

**Spec:** ../specs/2026-09-11-existing-text-design.md

## Constraints

- Preserve original files, font appearance, positioning and unrelated page content.
- Reject unsupported layouts/fonts and overflow explicitly; no hidden original text or automatic fallback.
- Do not add OCR/reflow/redaction. Keep unrelated code unchanged.
- Every derived source belongs to one session and remains usable by undo/redo.

## Tasks

- [x] Native capability and replacement: add text_edit.rs, Rust types/worker commands/Tauri endpoints, failing actual-engine tests, then validated immutable source derivation. Inspect/replace APIs share run indices and expected text. Confirm no-op geometry, ASCII mapping, overflow and saved extraction before accepting.
- [x] Page controls: add editable text overlay and modal with loading/empty/unsupported states, accessible focus/keyboard behavior and caller-owned apply/error state. Add targeted tests and connect PageView/DocumentViewport.
- [x] Workspace integration: adapter APIs and types; Edit text tool; commit chosen page source atomically while preserving overlays/rotation/history and tracking ownership. Test undo/redo, duplicate isolation, search and failure/stale behavior.
- [x] Integrated verification: all frontend/native regressions; source/editor rotation browser selection and actual packaged text replacement/save/reopen; independent PDF extraction and rendering checks. Review runtime changes independently and fix findings.
- [ ] Document support boundaries, update issue #8 and version/release metadata, package verified Windows executable, publish release after checks pass.
