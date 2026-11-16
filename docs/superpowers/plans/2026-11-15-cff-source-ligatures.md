# CFF source ligatures implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans inline; one fresh final whole-component review, no implementer agents.

**Goal:** edit existing verified CFF source ligatures with original physical glyphs and saved re-edit support.
**Architecture:** extend CFF proof into the existing bounded source_glyphs Codebook; expose bounded charstring width+ink metrics without duplicate outline work. Existing transaction/publication paths continue to enforce source identity and neighboring content.
**Tech Stack:** Rust/lopdf/local ttf-parser/PDFium, existing Python fontTools/MuPDF/pypdf fixture/audit tooling, TypeScript/Tauri desktop and CLI/MCP.
**Spec:** docs/superpowers/specs/2026-11-15-cff-source-ligatures-design.md

## Global constraints
No Git mutations, deletion of prior artifacts/private originals, new dependencies/private inputs/clipboard/version/package/launcher changes. Native/frontend/build/UI serial; one Cargo job, D:/Folio-build-cache/t3-8962a327 and its existing TEMP. Frontend min/max2workers/default5000ms. Reuse only existing public fixtures/fonts, retain licenses. Actual source semantics/code/glyph/matrix/width/ink and exact saved publication proof remain mandatory; no silent fallback.

## Review Focus
- A forged sequence must not relabel an ordinary glyph, use a zero-ink glyph, or borrow a conflicting Unicode subtable's mapping.
- Excluding an invalid sequence must preserve unrelated scalar eligibility, without making malformed CMap/charstrings safe.
- Raw SID/CID and wrapped programs must retain their actual PDF code/GID/matrix/advance and existing proof budgets; no unbounded outline replay.
- Source-bound edits, no-ops and saved re-edits must preserve following text, unrelated shared invocations, exact font programs and logical text order.
- Scalar form proof, explicit substitution, protected/tagged context, source ownership and draft/history behavior must retain their established guards.

### Task1: Fixture authority and RED proof
- [x] Generate renamed public-outline raw SID/CID/wrapped CFF ligature fixtures and independently recorded mappings/metrics/license.
- [x] Add focused verifier and real native transaction regressions; observe positive exclusion RED and negative/scalar controls.

### Task2: Native support and regression
- [x] Expose bounded width+outline metrics, preserve old API behavior and parser contracts.
- [x] Admit proved sequences through CFF codebooks with unchanged scalar/all-scalar policies and alias/ambiguity/budget checks.
- [x] Verify focused GREEN, full library correctness, affected optimized integration targets and parser tests; preserve all RED evidence.

### Task3: Compiled acceptance and review
- [x] Fresh full frontend and normal optimized GUI/CLI build with exact resources/source binding.
- [x] Actual compiled CLI/MCP/owned desktop edit/history/save/reopen/refusal checks; independent whole-page/code/font/text audits and corruption controls.
- [x] One final fresh scoped review, one RED/GREEN Important correction pass if needed; document evidence/limits and completion without closing the broad goal.
