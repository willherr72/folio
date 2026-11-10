# Text-removal desktop ownership implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline, task by task.

**Goal:** Bind source text-removal transactions to trusted desktop windows and expose a typed adapter for the composed UI workflow.

**Architecture:** A window registry retains private engine owner IDs behind independent public session handles. Per-window serialized operations and immediate destruction invalidation prevent late work escaping cleanup. Applied results remain window-owned for undo until explicit close or window destruction.

**Tech Stack:** Rust, existing Tauri worker and serde; TypeScript/Vitest. No dependencies.

**Spec:** docs/superpowers/specs/2026-10-30-redaction-host-design.md

## Global constraints

- 16 sessions/window, existing native global 16; 4096 applied IDs/window. Pending preview limits unchanged.
- No native owner UUID in request/response; trusted window comes from Tauri.
- No disk writes, original source closure, UI button, executable/package/parity claim.
- Preserve dirty worktree; record baseline/snapshots instead of commits/cleanup.
- One Cargo process, locked/offline and existing shared target. No blocked memory probes.

## Review focus

1. Old generation's queued operations after same-label registration: reject old work, preserve new work.
2. Lost Apply response: result remains owned until explicit close/window destruction.
3. Concurrent Apply and close: no double adoption, no permanently consumed capacity.
4. Wrong session/window identifiers: never consume a legitimate preview.
5. Bounded counts/invalid marks: refuse before creating additional native resources.

### Task 1: Native host and lifecycle

**Files:** new src-tauri/src/redaction_host.rs, redaction_host_tests.rs; modify lib.rs, redaction_preview.rs, redaction_selection.rs.

**Interfaces:** RedactionHost::new(PdfEngine), register_window(&str)->Result<(),String>, capture(&str)->Result<WindowHandle,String>, detach(&str)->Option<WindowHandle>, close_document(&str)->Result<(),String>. WindowHandle methods begin, preview(session,source,marks), bounds(session,preview,page,offset,limit), apply(session,preview), discard(session,preview), release(session), cleanup. Return existing native DTOs with camelCase Serialize. Mark/rect add Deserialize deny_unknown_fields.

- [x] Write real-worker lifecycle tests with self-contained simple PDF fixtures; use barriers around host operation closure for deterministic destruction races. Add capacity, one-shot Apply, cross-window/session and generation tests.
- [x] Run cargo test --lib redaction_host_tests; stub methods fail assertions (RED), not unresolved compilation.
- [x] Implement registry and WindowHandle serialized operations. Capture before spawn. Clear alive immediately on detach; clean pending owners and adopted IDs off UI thread. Remove closed IDs after successful engine close.
- [x] Run all redaction_ library tests; confirm real source preserved and result text removed. Record GREEN evidence.

### Task 2: Desktop commands and typed adapter

**Files:** new redaction_commands.rs, src/editor/text-removal.ts, tests/text-removal-adapter.test.ts; modify app.rs, lib.rs, src/editor/adapter.ts.

**Interfaces:** begin_text_removal_session; preview_source_text_removal(sessionId,sourceId,marks); text_removal_preview_bounds(sessionId,previewId,pageIndex,offset,limit); apply_text_removal_preview(sessionId,previewId); discard_text_removal_preview(sessionId,previewId); release_text_removal_session(sessionId). Window identity is an injected parameter only. TS optional capability uses corresponding camelCase methods.

- [x] Add adapter tests pinning exact command names, camelCase arguments, source operation semantics, no owner token and propagated errors; run RED.
- [x] Register host windows during setup, destroy immediately detaches and schedules cleanup; close_document uses host close. Commands capture WindowHandle before spawn_blocking. Add serde tests for output keys/input unknown fields.
- [x] Implement typed transport and FolioAdapter optional capabilities, no visible entry point.
- [x] Run focused frontend tests, full frontend tests/build, default native library regression and cargo check --bins. Capture final fingerprint.

### Task 3: Review and checkpoint

- [x] Freeze changed sources/tests and baseline delta; one fresh final reviewer checks ownership, races and integration boundaries. No implementation delegation.
- [x] Reproduce/fix material findings, rerun affected suite and refresh fingerprints if needed. Preserve limitations explicitly.
- [x] Record checkpoint, continuation and product-quality-loop status with exact evidence. Leave overall goal active; composition, visible marking and save-copy acceptance next.
