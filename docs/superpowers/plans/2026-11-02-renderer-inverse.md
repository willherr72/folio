# Reader Inverse Implementation Plan

> Execute inline with superpowers:executing-plans under standing authorization
> and the existing final phase3 review boundary.

**Goal:** Bound both supported readers' inverse-transform arithmetic.

**Architecture:** RendererTransform child module evaluates binary32 and
binary64-compatible interval paths; returns their coefficient-wise union.

**Tech stack:** Rust interval tests, actual MuPDF, pinned native PDFium probe.

**Spec:** docs/superpowers/specs/2026-11-02-renderer-inverse-design.md

## Constraints and review focus

Do not use the inverse for source selection. Refuse singular/uncertain/extreme
arithmetic. Tests cover negative determinants, cancellation, noncommutative
translations, uncertain inputs and the native near-zero branch. No commit,
package replacement or per-helper review; final phase3 review remains open.

- [x] Add reader_inverse stub and failing tests in redaction_render_inverse.rs
  and redaction_render_inverse_tests.rs; observe the fast harness red gate.
- [x] Implement signed interval division and both inverse evaluation paths;
  export owned known/interval cases including explicit refusals and run green.
- [x] Build a provenance-bound native PDFium source probe; audit actual MuPDF
  and exact rational values against exported intervals. Run ordinary native
  regression, compare prior artifacts and checkpoint exact final evidence.

Evidence: `artifacts/jpeg-reader-inverse-20261102/checkpoint.json`. The fast
harness passes 148 tests; ordinary native regression passes 284, with one
explicit skip. 832 owned input intervals yield 10022 native comparisons and an
independent saved-result exact-inverse check. All 583 prior exports are unchanged.
This completes only the inverse prerequisite. Fixed-point affine sampling and
the enclosing JPEG phase3/public acceptance plan remain open.
