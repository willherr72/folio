# Producer font identity implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Verify usable embedded producer fonts without conflating resource encodings.
**Architecture:** Preserve the existing proof map and guards; align resource names with PDFium and exclude unproven sparse codes from editing repertoire.
**Tech Stack:** Rust, lopdf, PDFium build 8044, existing DejaVu fixture.
**Spec:** docs/superpowers/specs/2026-10-26-producer-font-identity-design.md

## Global constraints

No dependency/UI changes, implicit substitution, raster guard relaxation, commits,
release or cleanup. Preserve original inputs and prior checkpoint. One Cargo at a time.

## Review focus

- Same native name/hash with conflicting mappings must refuse (task 2).
- Sparse missing codes used by source/replacement must refuse (task 1).
- Explicit zero-width ToUnicode contradictions must refuse (task 1).
- CID parent/descendant name disagreement must use descendant (task 2).
- Form all-scalar repertoire must remain strict (task 1).

### Task 1: Sparse editing repertoire

**Files:** src-tauri/src/embedded_font.rs; src-tauri/tests/embedded_text.rs.
**Interfaces:** existing verify_domain and public text APIs unchanged.

- [x] Add unit regression for O with a valid width and an unavailable l zero slot; O encodes, l cannot encode/decode; explicit map and all_scalars still refuse.
- [x] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib sparse_winansi -- --test-threads=1` and observe the width disagreement failure.
- [x] Skip only implicit editing WinAnsi zero-width mismatches; retain all other checks.
- [x] Run focused tests; add public fixture showing supported text edits while missing source/replacement codes refuse.

### Task 2: Renderer-compatible resource identity

**Files:** src-tauri/src/embedded_font.rs, source_text.rs, positioned_text.rs, text_edit.rs, text_paragraph.rs; src-tauri/tests/embedded_text.rs.
**Interfaces:** renderer_name(pdf, font) -> EngineResult<String>; matches_name(pdf, font, native_name, embedded) -> bool. FontProofs::find keeps its signature.

- [x] Add real same-program simple/CID resources and a CID parent-name mismatch regression, exercising inspection, single and paragraph editing. Verify same-identity conflicting mappings refuse.
- [x] Run new tests red against the old proof lookup.
- [x] Key proofs by renderer name, match exactly, and use shared name matching in all four source-operand consumers. Standard-font matching retains current behavior.
- [x] Run library, embedded_text, embedded_form_text, paragraph_font, styled_paragraph_multifont, form_font and equivalent_encoding targets. Fresh native build and original producer probe; record input hashes and actual blocker changes.
- [x] Independent final review, resolve demonstrated findings with regressions, record source hashes/evidence and roadmap. Preserve artifacts and worktree.
