# Image area redaction implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans.

**Goal:** Sanitize selected image samples per occurrence in source-owned area publication while preserving editable outside content.

**Architecture:** Strict owned image resources, area-only content admission, interval-bounded pixel selection, fresh resource publication with used-resource pruning.

**Tech Stack:** Existing Rust/lopdf/PDFium; independent pypdf/PyMuPDF acceptance.

**Spec:** `docs/superpowers/specs/2026-11-02-area-image-redaction-design.md`

## Constraints and review focus

8-bit DeviceGray/RGB raw or Flate, 16 million pixels/image, dimensions <=32,768, 64 MiB input and output sample budgets, 256 unique inputs, 4,096 occurrences, 8 million geometry comparisons and 64 million affected-occurrence pixels of sample work. No source/package replacement or public command. Focus on row orientation, shared-resource isolation, hidden original image payloads, outward-rounded boundaries and pre-allocation limits.

## Task 1 — Owned samples and conservative pixel selection

New files: `redaction_images.rs`, `redaction_images_tests.rs`, `redaction_image_pixels.rs`, tests. Extend `redaction_area_paths.rs` only with a private transform wrapper reusing its checked interval operations.

- [x] Add failing admission/sanitization tests; establish red evidence.
- [x] Implement bounded decode, owned sample hashing, pixel selection and fresh stream construction; verify positives and refusal paths.

## Task 2 — Actual PDF integration and verification

Files: `redaction_content.rs`, `redaction_core.rs`, `redaction_ranges.rs`, `redaction_area_content.rs`, `redaction_selection.rs`, `redaction_space_probe.rs`, private module declarations and owned integration fixtures.

- [x] Add failing shared-occurrence and mixed text/path/image native tests.
- [x] Implement area-only image preparation, source-bound glyph/range routing, occurrence sanitization and final used-resource pruning.
- [x] Run serial redaction native gate. Independently decode and render saved output, testing corruptions and original/source invariants.
- [x] Freeze the scoped delta for one fresh review, correct demonstrated findings, and checkpoint verified evidence and open limits. Keep the broad goal active.

Outcome: final gate 154 passed, zero failed, one explicit skip. Independent saved-output sample/graph/glyph/raster checks and five PDF corruption controls passed. Geometry has 25 focused tests and 240 exact-rational cases. The reviewer-found identity-parameter mismatch and coordinator-found repeated-image workload gap have observed red/green regressions. See `artifacts/area-image-redaction-20261102/review-disposition.md` for remaining coverage and product limits.
