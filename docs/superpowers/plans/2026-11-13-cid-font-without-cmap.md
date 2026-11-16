# CID font without internal cmap implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline in the existing managed worktree. No commits, cleanup or package selection. User opted out of routine questions. One final fresh review after completed native/build/runtime evidence.

**Goal:** Make proved map-free CID TrueType text and its standard-font neighbors editable, with original-font preservation and explicit substitution.
**Architecture:** Optional internal cmap solely for bounded existing CID editing; require source operands and preserve encoding for new admission. Existing PDF mapping/width/outlines and publication proofs stay authoritative.
**Tech Stack:** Rust/ttf-parser/lopdf/PDFium, existing Tauri CLI/GUI; public fontTools-derived DejaVu fixture and independent PyMuPDF audit.
**Spec:** docs/superpowers/specs/2026-11-13-cid-font-without-cmap-design.md

## Global Constraints

Preserve parent source6fe8de0e9422fd1c6c5b3e0e0693464b0f305255fecf402ee2b3a25c75d237c4, selected050 and reviewed051; no private fixture/clipboard/promotion/commit/delete/merge/push. Native testsonejob,D:cache/temp, serial processes. Library opt0 correctness results must not become performance/ordinary optimized claims. No new portable qualification. One final fresh review only.

## Review Focus

- Malformed-present internal cmap must not become an absent-map exception.
- PDF code/glyph mapping, Unicode ambiguity and finite advance evidence must survive every new admission.
- Excluded or invisible source operands must fail exact binding rather than disappear through extraction.
- Save/reopen/re-edit must retain original glyph programs/maps and source order; standard-font neighbor isolation matters.
- Paragraph/native GUI/automation should agree with direct verifier admission; complete form proof and simple encodings retain strictness.

### Task 1: Mapping proof and regressions

**Files:** src-tauri/src/embedded_font.rs; new source unit test module if needed; src-tauri/tests/cid_font_without_cmap.rs; tests/fixtures/embedded-text/DejaVuSerif-cid-no-cmap.ttf (derived from existing public font with fontTools, retaining glyph IDs).
**Interfaces:** VerifiedFont existing fields/API unchanged. For truly absent cmap + CID editing, set requires_source_operands and preserve_source_encoding. Original-map, legacy/simple/all_scalars behavior unchanged.
- [ ] Baseline source/test-input capture and public font fixture provenance before production edits.
- [ ] RED unit proof for Identity/explicit shifted map/default; refusal controls for malformed cmap, missing ToUnicode, unsupported simple/complete form, bad metrics/GID/odd mapping/ambiguity.
- [ ] GREEN minimal verifier change; focused unit run and original checks.
- [ ] Native public integration: no-op/delete/growth/save/reopen/re-edit/font-map bytes; explicit substitution; standard neighbor; paragraph; source integrity and refusal controls. Diagnose further failures before extending code.
- [ ] Complete library correctness plus affected ordinary optimized integration suites, with source/input guards; no all-project claim unless actually run.

### Task 2: Compiled workflow and independent output

**Files:** artifacts/cid-font-without-cmap-20261113/{baseline,source,tests,build,desktop,helpers}.
**Interfaces:** Exact declared native product delta; frontend unchanged. Current normal optimized GUI/CLI/source/resources bound to the accepted source.
- [ ] Verify frontend equality; reuse prior1663frontend results only with explicit unchanged-input qualification, or run fresh if inputs affect frontend behavior.
- [ ] Normal optimized build(s); public map-free source original-font edit, explicit substitute, paragraph and neighbor workflow, save/reopen and independent text/raster/resource comparison. No clipboard.
- [ ] Preserve first failure and all original sources, selected pointer, packages and parent checkpoint.

### Task 3: Final review and recorded component

- [ ] Freeze exact source/test/build/runtime/independent evidence and dispatch one fresh final reviewer.
- [ ] Re-grade findings, one Important RED/GREEN pass if required; explicitly disposition declined judgments and coverage limits.
- [ ] Verify component checkpoint, leave broad goal active and selected builds unchanged.
