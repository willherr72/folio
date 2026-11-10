# Opaque Graphics Removal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline. No implementer delegation.

**Goal:** Preserve bounded opaque saved graphics states while removing selected text.
**Architecture:** A reference-free state registry and page resolver feed canonical gs names into existing removal; state digests bind selections and output dictionaries are shared.
**Tech Stack:** Rust, lopdf, SHA256; Python independent readers.
**Spec:** docs/superpowers/specs/2026-10-30-opaque-graphics-removal-design.md

## Global Constraints
User authorizes autonomous execution. Existing isolated dirty worktree; preserve unrelated files, private PDFs, prior artifacts and executables. No commit/release or user app restart. One Cargo job/process; offline locked. Keep source-defined independent masks. No public API/frontend change or general transparency admission. Registry limits256document/64page, reference depth64, ten scalar dictionary keys per spec.

## Review Focus
- Partial dictionaries and gs inside BT/q/Q retain inherited semantics (task1).
- Indirect scalar/reference cycles and aliases cannot escape limits (task1).
- Changed state invalidates previous selection identity (task1).
- Multiple pages and distinct state resources do not multiply shared output graphs (task1).
- Independent output masks cannot be enlarged by the candidate (task2).

### Task 1: Preserve opaque graphics state
Files: new src-tauri/src/redaction_graphics.rs and redaction_graphics_tests.rs; modify redaction_content.rs, redaction_core.rs, lib.rs.
Interfaces: graphics Registry default; PageStates::new(doc,resources,&mut Registry), select(alias)->EngineResult<usize>, states Vec<Arc<State>>. State stores normalized Dictionary and digest. Publisher publishes one state per digest. canonicalize adds &mut PageStates argument. Page stores states.
- [x] Write failing tests through existing inspect/remove APIs: producer-like state, q/Q and inside-BT gs, partial dicts, publication/sharing, stale IDs, cycles/missing/unknown resources and opacity rejection. Run cargo test --lib redaction_graphics -- --test-threads=1 (expected new positive tests fail).
- [x] Implement bounded state registry/resolver and deterministic digest, canonicalize gs, hash states and publish resources. Run same command (expected pass).
- [x] Add boundary tests for64page/256document and repeated aliases/pages, exact source coordinates and saved re-edit. Run cargo test --lib redaction_ -- --test-threads=1 (expected all pass).

### Task 2: Verify saved output and producer limitations
Files: artifacts/opaque-graphics-removal-20261030 scripts/reports, test corpus emitter in new test file.
- [x] Emit synthetic original/removed/re-edited PDFs create_new only. Audit via MuPDF/PDFium: retained text positions and artwork, source-only masks, wrong-output negative controls. Expected both readers preserve all pixels outside original selected-glyph region.
- [x] Probe original quiz locally through strict prepare/inspection, record only structural counts/error class and original hash; do not persist its text. Expected honest pass/refusal record, not forced acceptance.
- [x] Run full native --lib and --bins check with build-source fingerprint, no concurrent edits. Expected zero failures, existing skips documented.
- [x] Freeze scoped files/evidence and request one final fresh review. Correct material defects once with RED/GREEN and affected regression. Record checkpoint/continuation/roadmap. Native milestone only; current runnable desktop checkpoint retained until a later integrated build.
