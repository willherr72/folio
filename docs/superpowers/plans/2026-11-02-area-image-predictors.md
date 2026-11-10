# Image predictor implementation plan

Use superpowers:executing-plans inline under the standing autonomous authorization.

Spec: `docs/superpowers/specs/2026-11-02-area-image-predictors-design.md`.

- [x] Add image-level positive and refusal regressions and observe the current predictor rejection.
- [x] Implement strict parameter validation and in-place TIFF/PNG reconstruction, leaving the generic zlib decoder unchanged.
- [x] Run serial native redaction regressions; independently audit generated predictor PDFs and their actual saved/re-edited outputs.
- [x] Freeze scoped source, obtain one fresh review, correct findings, and checkpoint evidence and limits.

Files: `src-tauri/src/redaction_images.rs`, a private predictor helper, image decoder tests and owned area-image native tests. Artifact root: `artifacts/area-image-predictors-20261102`. Preserve prior evidence and accepted 0.30 package. No broad release or parity claim.

Outcome: 158 focused native passes, zero failures, one explicit skip; 21 predictor outputs plus two prior-format outputs independently checked (74,520,000 rendered RGB channels), seven corruption controls rejected. One final review found no demonstrated P0–P3 defects. Its optional baseline-provenance recommendation was incorporated into the audit and rerun without product changes. Broader compatibility and desktop integration remain open.
