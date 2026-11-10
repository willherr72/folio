# Renderer Sizing Implementation Plan

> Execute inline with superpowers:executing-plans. This continues phase3 of
> jpeg-area-redaction; final fresh review remains at its integrated boundary.

- [x] Add red tests for device-based PDFium sizing, partial edges, MuPDF extent
  lower bounds and invalid inputs in redaction_render_sizing.rs; register module.
- [x] Add red/green tests and implement conditional outward stretch composition
  in redaction_sampling.rs and redaction_sampling_tests.rs.
- [x] Export actual helper results. Verify real PDFium candidate decoding/cache
  behavior and MuPDF tile sizes; compare native source envelopes with scaling.
- [x] Run serial native redaction regression, preserve previous outputs and save
  checkpoint/limits. No package/public-admission changes or commits requested.
