# Complete-source reusable form text implementation plan

> Use superpowers:executing-plans inline, with one final scoped review.

**Goal:** Edit a reusable form occurrence while preserving its connected PDF.
**Architecture:** Source-authorized private projection, annotation-aware legacy
form proofs, immutable complete-source publication with occurrence mapping.
**Tech stack:** Existing Rust/PDFium/lopdf engine and Python independent readers.
**Spec:** docs/superpowers/specs/2026-10-24-occurrence-form-design.md.

Global constraints: preserve existing permission, resource, glyph and pixel gates;
no changes to legacy admission; no source overwrites, commits or release claims.
Review focus: annotation geometry/fringes, AP resources shared with content,
private source lifetime, source-only restrictions, retained fields across duplicates.

- [x] RED connected nested-form fixture in tests/page_occurrence_replace.rs,
  proving current annotation refusal before adding new methods.
- [x] Add occurrence_form.rs request/worker/public native methods and engine
  dispatch. Internal form_text mode retains original preflight and permits
  annotations only through the complete-source transaction; validate collisions.
- [x] Test authored reference/unchanged pixels, saved/reopened invocation identity,
  duplicates, shared fields, restrictions, permissions and cleanup; expand physical
  matrix and verify saved files through independent readers.
- [x] Run native regressions and scoped review. Address demonstrated defects;
  record exact evidence, hashes and remaining desktop integration work.

Accepted native evidence: artifacts/occurrence-form-20261024/checkpoint-native-accepted/
verification.json. 357 regression passes / 4 opt-in ignored / 18 suites, plus two
supplementary passes. Independent 72-case authored-reference and structural checks
passed. One review found no concrete defects. No desktop executable or parity claim.
