# Existing Unshaped Unicode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans inline.
> Standing autonomy replaces routine confirmation. One fresh final scoped review
> after actual acceptance; no per-task agents or review loops.

**Goal:** Edit verified existing Greek/Cyrillic and other already-supported
unshaped BMP text while preserving the original font and document.

**Architecture:** Reuse the imported-font character classifier, broaden embedded
font repertoire proof, and apply the same policy at every existing-text consumer.
Keep nominal standard-font Latin admission separate and every preservation check.

**Tech Stack:** Existing Rust, lopdf, ttf-parser, PDFium, Tauri and Python auditors.

**Spec:** docs/superpowers/specs/2026-11-11-existing-unshaped-unicode-design.md

## Global Constraints

Keep4096UTF8bytes, exact font/glyph/width/source/context/neighbor checks. No implicit
font fallback or new shaping dependencies. Accepted043/source9021/v8DLL3bcf stays
immutable and selected. Preserve all attempts/ledgers. One Cargo job, D:build/temp
cache, serialize native/build/UI; distinguish Folioopt0 correctness from optimized
integration/builds. No commits/push/merge/publication or user-app auto-launch.

## Review Focus

- Broad font CMaps containing unused shaping/combining entries cannot authorize unsupported source operands.
- Nominal standard-font and malformed/missing glyph/width cases retain exact eligibility and atomic refusal.
- Single, grouped, paragraph, saved-memory and Form routes cannot silently narrow or expand one another's repertoire.
- Unicode editing keeps original code/glyph identity, surrounding text order/geometry and saved re-editability at rotations and UserUnit1.7.
- Desktop/automation acceptance must use the new actual binary; preserved043 evidence cannot stand in for new native/runtime results.

### Task 1: source font and single-run support

**Files:** fonts.rs, embedded_font.rs, text_edit.rs and new
src-tauri/tests/existing_unshaped_unicode.rs; component-owned baseline/evidence.
**Interfaces:** Existing Latin predicate stays for standard fonts; new shared
unshaped predicate drives embedded proof and original single-run replacement.

- [ ] Capture baseline/current043 authority and source/test input manifests. Expected: accepted package untouched, isolated workspace, no competing native/UI.
- [ ] Add real embedded DejaVuSans fixtures for Greek/Cyrillic/mixed/symbol text and source-preserving single-run edit/save/reopen/re-edit/no-op tests. Run against current implementation. Expected: actual Latin-only admission RED, not build/fixture errors.
- [ ] Expose the existing imported-font classifier; add shared embedded unshaped eligibility while retaining nominal Latin policy. Update single-run diagnostics and CMap filtering. Expected: new single-run tests GREEN, unsupported source/replacement/missing glyph/width controls still refuse.
- [ ] Verify rotations/units, immutable font programs/maps, exact neighbor bounds/style and source bytes; run affected font/text suites. Expected: original-font Unicode edits are proved and existing Latin behavior passes.

### Task 2: all editing consumers and saved memory

**Files:** text_group.rs, form_text.rs, paragraph_layout.rs,
styled_paragraph_plan.rs, styled_paragraph_memory.rs, text_substitution.rs,
source_text.rs and targeted integration tests.
**Interfaces:** All eligible existing-text and substitute-font routes consume
Task1unshaped policy; standard_font.rs retains nominal Latin proof.

- [ ] Add grouped, uniform/styled paragraph, explicit substitute and reusable Form Unicode regressions, including saved re-edit. Expected: remaining actual Latin guards reproduce RED.
- [ ] Replace only inappropriate Latin guards/diagnostics with shared unshaped policy; preserve all source ownership/layout/publication checks. Expected: grouped/paragraph/Form routes GREEN and no silent fallback.
- [ ] Add missing mapping/glyph/width, combining/bidi/complex-script/emoji and4096byte/line controls. Expected: unchanged source bytes and no candidate on invalid requests.
- [ ] Run the combined targeted regression, including legacy Latin standard fonts and embedded CMap/source contexts. Expected: all pass with exact input/source receipts.

### Task 3: independent native and visible acceptance

**Files:** component-owned native/build/desktop/CLI/MCP/audit/review/checkpoint
artifacts; existing text/font support docs and quality status.
**Interfaces:** Current source/test manifests bind all fresh tests and actual new
normal optimized GUI/CLI candidate; accepted043stays separate.

- [ ] Run complete fresh native library correctness and optimized integration/bin/doc gates with full target/log membership and source/input guards. Expected: zero failures, explicit skips and original profile meanings.
- [ ] Build normal GUI/CLI, run actual Unicode CLI/MCP and desktop preview/cancel/apply/undo/redo/save/reopen/re-edit, and independent MuPDF/source glyph/font/pixel audits. Expected: current new executable identities and protected surrounding content.
- [ ] Execute frontend validation appropriate to native diagnostics/IPC behavior; verify unsupported draft retention and source/memory limits. Expected: no UI-hidden Latin restriction.
- [ ] One fresh final scoped review; one consequential observedRED/GREEN fix pass, no re-review. Seal complete required evidence with corruption/relocation checks and document actual scope. Expected: no open consequential finding, no changes to accepted043launcher, broad goalactive.
