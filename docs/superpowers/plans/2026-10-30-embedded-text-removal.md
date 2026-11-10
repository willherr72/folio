# Embedded text removal implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline.

**Goal:** Remove selected existing embedded TrueType text while retaining source codes, typography and following geometry.
**Architecture:** Bounded owned font registry feeds canonicalization and glyph accounting; existing native preview/history/Save remains the end-to-end route.
**Tech Stack:** Rust/lopdf/ttf-parser/PDFium; existing React desktop, Vitest and independent PDF readers.
**Spec:** docs/superpowers/specs/2026-10-30-embedded-text-removal-design.md

## Global constraints

- 64 used embedded resources,16MiB program,2MiB maps,64MiB aggregate normalized font payload,100000 graph nodes,64levels. Charge before copies.
- Keep strict current page/content admission and existing output/session limits; no weaker alternate font proof.
- Glyph scalar ordinals, exact source bytes, word spacing only single-byte32, retained code/Unicode/origin/end checks within0.002pt.
- Shared font programs remain Arc-owned and published once. No arbitrary source object graph import.
- Existing isolated dirty worktree and artifacts preserved; no commits/releases/original overwrite/user-app restart. One Cargo at a time.

## Review focus

1. Multibyte Unicode before/within marks must not turn glyph ordinals into byte slices.
2. Composite code32 and Unicode space must not gain single-byte PDF word spacing.
3. Aliases/repeated pages must not amplify retained font memory or output graph.
4. Malformed/unknown/cyclic font resources must not bypass the existing proof or resource budget.
5. Native extraction can omit glyphs or choose another encoding; source/native binding and independent retained-pixel checks remain mandatory.

### Task1: Bounded font resources and scalar accounting

**Files:** new src-tauri/src/redaction_fonts.rs; modify redaction_core.rs, redaction_content.rs, redaction_ranges.rs, redaction_selection.rs, embedded_font.rs, text_edit.rs, engine.rs, lib.rs; new redaction_embedded_tests.rs.
**Interfaces:** engine::redaction_font_codes(&Document,&Dictionary)->EngineResult<(usize,BTreeMap<u16,(char,f32)>)> bridges existing proof. redaction_content::TextFonts has select(&mut self,alias:&[u8])->EngineResult<usize> and decode(&self,index:usize,bytes:&[u8])->EngineResult<String>. Registry loads source-bound Arc fonts; Font provides code_len, decode, glyph(code), digest and publish into fresh Document with deduplication. Canonicalizer retains operand bytes and tracks font index through q/Q.

- [x] Add a native fixture using DejaVuSans existing licensed test font, simple WinAnsi and Identity-H resources, mixed standard text. RED: inspect and partial removal of accented source, re-open exact retained text/code/origins.
- [x] Implement bounded role-specific normalization, existing-proof bridge, registry/cache and source digest. Reject unknown fields, malformed streams, incomplete/unproved codes. Keep standard fonts unchanged.
- [x] Replace ASCII-byte assumptions with glyph ordinals in canonicalization, ranges and selection merging. Preserve one/two-byte operands and correct Tw compensation. Publish owned font graph once and compare retained Unicode too.
- [x] Focused cargo test --lib redaction_ -- --test-threads=1: require new tests GREEN and old behavior unchanged.

### Task2: Native proof and resource regressions

**Files:** same new native test file; engine test registration; artifacts/embedded-text-removal-20261030/ fixture/audit helpers.
**Interfaces:** existing WorkerRuntime inspect/preview/Apply and TextRange functions, no new public APIs.

- [x] Add tests for UTF-8 preceding selected text, Tj/TJ, mixed fonts, Tc/Tw/Tz and q/Q, code32 mapped to nonspace, non32 mapped to space, stale font maps/widths, incomplete codes, partial/ambiguous mapping and malformed graphs.
- [x] Test shared font across aliases/pages deduplicated; cyclic/unknown/oversized resources and aggregate budget refuse. Use bounded small injected budgets where allocation at a real limit would be wasteful, plus production-constant assertions.
- [x] Actual native mark/preview/Apply with saved embedded output; save/reopen/re-edit and source originals remain unchanged. Independent readers compare text, source-derived glyph masks and outside pixels; bad-output controls must fail.
- [x] Run focused then full library and binary checks, existing full frontend/build as relevant; preserve exact input hashes and failures.

### Task3: Corrected desktop, review and checkpoint

- [x] Build stable local custom-protocol candidate; native desktop embedded Preview/Cancel/Apply/Undo/Redo/save/reopen acceptance. Preserve user app.
- [x] One fresh final reviewer of frozen scope/evidence, then coordinator material corrections RED/GREEN + full affected suites; no re-review.
- [x] Record scope, proof limits, review dispositions and continuation in checkpoint/ledger/roadmap. Overall Adobe/Foxit goal remains active.

Scoped tasks are complete; explicit remaining coverage limits are in artifacts/embedded-text-removal-20261030/checkpoint.json. No exhaustive compatibility or release claim.
