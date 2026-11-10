# JPEG Source Envelope Implementation Plan

> Execute inline using superpowers:executing-plans. Continue the existing phase3
> boundary; final fresh review belongs to the completed integrated phase.

Design: ../specs/2026-11-02-jpeg-source-envelope-design.md.

- [x] Red/green tests in redaction_sampling_tests.rs for all-window bounds,
  partial edges, invalid grids/factors, and immutable source coverage.
- [x] Implement SampleRegions::source_envelope and the Coverage adapter in
  redaction_sampling.rs and redaction_jpeg_coverage.rs.
- [x] Export source-envelope configurations from actual production helpers in
  redaction_jpeg_decoder_tests.rs, including the owned256x192 JPEG. Audit exact
  crop/box support and actual compressed-image MuPDF downsampling/cache paths.
- [x] Run serial native redaction regression, compare previous outputs, and
  save checkpoint/limits. Do not enable public JPEG admission or change packages.
