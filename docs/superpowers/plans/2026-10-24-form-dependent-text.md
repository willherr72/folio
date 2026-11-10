# Consecutive form text implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline.

**Goal:** Edit consecutive form text pieces while retaining following placement.
**Architecture:** Existing stream isolation plus verified cursor compensation;
preserve source fonts and state, retain native/independent appearance proofs.
**Tech Stack:** Rust, lopdf, PDFium, Python/MuPDF/pypdf, Tauri/WebView2.
**Spec:** `docs/superpowers/specs/2026-10-24-form-dependent-text-design.md`

## Constraints and review focus

Keep source files/previous checkpoints and all appearance/collision guards.
No whole-form rewrite affecting shared instances, invisible retained original text,
or copied parser matrices. Inspect font-width rounding, Tc/Tw encodings, later line
matrix operations, numeric-only object mapping and repeated correction growth.

## Task 1: Admission and end-cursor preservation

- [x] Add `src-tauri/tests/form_dependent_text.rs` using existing authored form/font
  support. Reproduce current rejection with first/middle/last dependent pieces.
- [x] Extend `form_text_flow.rs` to retain known-position admission across shows.
- [x] Add a focused `form_text_advance.rs` helper for source-bound width/state math,
  consumed by `form_text.rs` publication. Keep existing object paths stable.
- [x] Verify spacing boundaries, explicit TJ slots, quote/line movement, multiple
  fonts and substitution; retain malformed/clip/collision refusals.
- [x] Bound repeated corrections and save/reopen/reference behavior.

### Renderer prerequisite discovered during Task 1

- [x] Reproduce fractional simple/CID width truncation without Folio in
  `scripts/probe-pdfium-font-advances.py`; retain integer controls and MuPDF references.
- [x] Add a separate pinned precise-horizontal-advance source patch and build candidate.
- [x] Pass normal direct probe and candidate native/independent tests before adoption.
- [x] Update dependency recipe/variant/provenance and repeat broad renderer regressions.

## Task 2: Independent and product acceptance

- [x] Generate owned public fixtures and verify native outputs with MuPDF/pypdf.
- [x] Run relevant library/form/positioned tests; diagnose any exposed metric issue.
- [x] One scoped final review; one regression-verified correction pass.
- [x] Build with renderer-aware fingerprint and self-contained resources, perform
  actual Windows edit/undo/save/reopen, and inspect independent outputs.
- [x] Record checkpoint, remaining compatibility limits and next gaps; goal active.

Acceptance evidence: `artifacts/form-dependent-text-20261024/verification.json`.
The full run recorded 710 passes, two failures and seven skips. Complete corrected
test groups subsequently passed (3 embedded-form tests and 9 grouped-text tests),
covering 712 distinct native tests in combination. No product change followed the
desktop build. Ordinary spaces are now intentionally supported. Two embedded-font
split boundaries are conservatively refused because merging changes native pixels;
five other boundaries pass. The existing exact appearance guard remains unchanged.
