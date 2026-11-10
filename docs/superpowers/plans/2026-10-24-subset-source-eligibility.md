# Embedded subset source eligibility implementation plan

> Use superpowers:executing-plans inline with one final scoped review.

**Goal:** Edit valid embedded-subset source text despite generated reader spaces.
**Architecture:** Reuse bounded source operands; centralize complete run-property
validation and apply it to local edit clones while preserving raw snapshots.
**Tech stack:** Rust/PDFium/lopdf, existing Tauri Windows acceptance.
**Spec:** docs/superpowers/specs/2026-10-24-subset-source-eligibility-design.md.

- [x] Add a failing CID subset test with no space mapping and generated separator.
- [x] Factor property validation in text_edit.rs and use source-aware local
  eligibility for listing, no-op and changed-run admission.
- [x] Verify missing actual glyphs, unsafe appearance/transforms, original font,
  saved/reopened/re-edited data, rotations and raw paragraph/group compatibility.
- [x] Scoped review, appropriate regression, staged build, Windows editing and
  independent saved-output references; record limitations and hashes.

Do not clear reasons by string, mutate native snapshots, loosen font proofs,
change saved schemas, overwrite originals, or start a release. Use a fresh
artifact root `artifacts/subset-source-eligibility-20261024`.

Review P2 substitution failure reproduced and corrected with verified source clone
at operand matching. Final embedded15pass; earlier broad261native/3ignored. Native
24saved outputs and Windows12outputs independently match text, 1x/2x pixels and
font bytes. Final source/exe binding in checkpoint-final/verification.json.
Raw group/paragraph eligibility and untested combinations remain explicit gaps.
