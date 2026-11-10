# Comment Fidelity Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline; one focused fresh review at completion.

**Goal:** Preserve standard sticky-note metadata and provide useful native editing controls without breaking old recovery drafts.

**Architecture:** Normal opens keep Text annotations native. Per-source recovery mode preserves legacy overlay ownership. Immutable native updates change Contents/Rect/C or remove notes/popups; React applies each full-source update as one undo step.

**Tech Stack:** Existing Rust/lopdf/PDFium, Tauri, React/Vitest and repository desktop acceptance tools.

**Spec:** docs/superpowers/specs/2026-10-23-comment-fidelity-design.md

## Global constraints

No new dependencies or private comment ownership format. Preserve originals and existing packages. Annotation permissions and signature guards remain native. New recovery annotation import version3 requires explicit source mode. No approval pauses or commits.

## Review focus

- Old recovery comments that were edited/deleted must not duplicate or return.
- Mixed legacy/native sources must retain their own mode after replacement and another checkpoint.
- Position fields must match intrinsic crop/rotation and added workspace rotation.
- Deletion must not leave associated popups visible or break reply relationships.
- Comment controls must preserve unknown metadata and custom appearance streams.

### Task 1: Native preservation and recovery modes

Files: engine.rs, recovery.rs, source constructors, native_comments.rs, tests/comment_fidelity.rs, annotations/native_comments/form/recovery tests.

- [x] Reproduce untouched-comment metadata loss with a failing native export test and inspect Folio-created dictionary output.
- [x] Add normal native Text admission and per-source legacy_comment_import bool; recovery snapshot Option<bool> defaults only for old manifests, explicit on version3. Add native source-mode query and inherit mode in full-source replacements.
- [x] Add migration regression for old edited/deleted overlays, mixed modes, and another recovery generation. Update legacy tests to assert equivalent visible/native behavior rather than obsolete overlay representation.
- [x] Verify untouched metadata/AP/popup fidelity, source immutability, permissions and recovery; run focused native suites.

### Task 2: Native comment mutation controls

Files: native_comments.rs, engine.rs, app.rs, tests/comment_fidelity.rs; frontend types/adapter/native-comments.ts/NativeCommentDialog/App and tests.

Interfaces: extend EditNativeCommentRequest with optional position:Point and color:string. Add DeleteNativeCommentRequest {pageIndex,annotationIndex,expectedText} and delete_native_comment/source adapter command. NativeComment adds optional color, canChangeColor and deleteReason. Runtime mutations return DocumentInfo through the existing full-source ownership helper.

- [x] Write failing native tests for movement across crop/rotations, AP-preserving text/position edits, standard color changes, custom-AP color refusal, deletion/popups/duplicate placements, reply-parent refusal and stale requests.
- [x] Implement bounded native mutations and immutable replacement with inherited policy/mode, preserving all unrelated fields. Delete only associated page placements; verify serialized results before publishing ownership.
- [x] Write failing frontend tests for displayed-coordinate conversion, apply/undo/recovery ownership, delete/error/orphan paths, and custom-appearance/read-only controls.
- [x] Implement numeric position/color controls and deletion in native comment dialog, then run focused UI tests and production build.

### Task 3: Acceptance, review and documentation

- [x] Run full appropriate frontend/native verification and Windows build.
- [x] Exercise standard F4 and fixed-icon notes in real desktop save/reopen, movement, deletion and undo. Independently inspect metadata, appearance bytes and unaffected page pixels.
- [x] Run one fresh read-only focused review; fix important findings with regressions and rerun affected checks.
- [x] Record authoritative evidence, remaining gaps and a new test executable; preserve the previous verified checkpoint. Keep overall parity goal active.
