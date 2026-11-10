# Edited-document snapshot implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline.

**Goal:** Prepare and own the current edited PDF before text marking, without bypassing unsaved edits or hiding Save composition omissions.

**Architecture:** Reuse the Save compositor and its resource capture. The native text-removal store owns prepared documents and input provenance, sharing retained output capacity with previews. Host sessions expose preparation through the existing trusted window boundary.

**Tech Stack:** Existing Rust/Tauri/PDFium/lopdf and TypeScript/Vitest; independent local Python PDF readers for exported test artifacts.

**Spec:** docs/superpowers/specs/2026-10-30-redaction-composition-design.md

## Global constraints

- 1..1000pages, flatten=false, at most10000total overlays; bounded input parse.
- At most4prepared sources,15minute TTL,64MiB each; shared128MiB retained prepared/preview bytes. Existing4pending preview and32MiB glyph limits retained.
- Preserve original input sources and reject unadopted/prepared dependencies.
- No silent extra content stripping, disk publication, UI entry, package or parity claim.
- Preserve inherited dirty work; one Cargo at a time, locked/offline/shared target. Snapshots replace commit/cleanup scripts.

## Review focus

1. Original input closes or loses rights after preparation: Preview/Apply must refuse and release only owned resources.
2. Prepared IDs reused/swapped or cross-owner requests: allocation identity and owner checks cannot close unrelated documents.
3. Save page-import metadata loss: receipt must be captured before loss and survive into removal previews.
4. Queued custom fonts/images disappear from frontend: native capture retains resources through composition.
5. Duplicate/cropped/rotated pages: working copy order/geometry must be correct and unsupported strict removal must leave the copy intact.

### Task 1: Native preparation and lifecycle

**Files:** create src-tauri/src/redaction_composition.rs and redaction_composition_tests.rs; modify engine.rs and redaction_preview.rs.

**Interfaces:** TextRedactionSource{document:DocumentInfo,omitted_metadata:Vec<&'static str>}; PdfEngine::prepare_text_redaction_source(&self,owner:&str,request:ExportRequest,bookmarks:Option<Vec<DocumentBookmark>>)->EngineResult<TextRedactionSource>. Request::Prepare carries captured page_image_render::Resources. Shared compositor helper returns (Vec<u8>,bool retained_complete_source), existing compose_pdf_monitored wraps and discards route receipt.

- [x] Write initial native tests and nonfunctional prepare stub. RED: ordered duplicated pages and added text appear in prepared output; source bytes remain unchanged.
- [x] Implement shared compositor route receipt, bounded input preflight, resource capture/physical conversion and prepared source registration. Return bookmarks from actual composed bytes and omission categories only for page import.
- [x] Extend store budgets, Preview/Apply provenance validation, expiry/release/source-close/dropped-reply cleanup. Remove records before invoking hooks; refuse nested prepared or pending sources.
- [x] Add native regressions for duplicate-only removal, standard text/ink/annotations/forms, crop/rotation/UserUnit, metadata receipts, stale input/owner/allocation/capacity, resource leases and replies. Compare prepared rendering with page-plan rendering, and assert extracted contents/order/geometry.
- [x] Run cargo test --lib redaction_ -- --test-threads=1. Expected: all focused tests pass. Preserve all failures and corrections.

### Task 2: Desktop preparation transport

**Files:** modify redaction_host.rs, redaction_commands.rs, app.rs, src/editor/text-removal.ts, tests/text-removal-adapter.test.ts; extend native host tests where needed.

**Interfaces:** WindowHandle::prepare(session,request,bookmarks)->TextRedactionSource; prepare_text_removal_source(sessionId,request,bookmarks?) command; optional prepareTextRemovalSource(sessionId,pages,bookmarks?) adapter returns {document,omittedMetadata}. No owner parameter or automatic Apply.

- [x] Add failing transport assertion for exact request envelope and absent-versus-empty bookmark semantics.
- [x] Implement host/command/adapter and native window destruction preparation cleanup test.
- [x] Run focused adapter tests and TypeScript/Vite build. Expected: pass; no visible feature entry introduced.

### Task 3: Fidelity audit and final review

- [x] Export self-contained original/composed/redacted fixtures. Check local independent-reader text, page geometry, retained annotations/forms/bookmark targets and per-page renders; preserve originals and receipts. Include negative checks that detect swapped duplicate edits or omitted overlay text.
- [x] Run full library sequentially, then frontend checks; keep a product fingerprint across gates. No concurrent heavy regression runs.
- [x] Freeze changed scope/baseline/evidence; one final independent review. Reproduce and fix material findings, no re-review.
- [x] Record checkpoint/continuation/roadmap and exact limitations. Goal remains active; UI/history/save-copy next.
