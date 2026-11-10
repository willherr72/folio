# Cropped JPEG Sample Support Implementation Plan

> Execute inline using superpowers:executing-plans. This continues phase 3 of
> 2026-11-02-jpeg-area-redaction.md; final integrated review remains at that
> phase boundary. No commits or package changes are requested.

Design: ../specs/2026-11-02-jpeg-crop-support-design.md.

- [x] Add red tests in redaction_sampling_tests.rs and
  redaction_jpeg_coverage_tests.rs for half-open crop intersection, empty results,
  invalid windows and rational partial-grid mapping.
- [x] Implement SampleRegions::crop and into_source_regions in
  redaction_sampling.rs, and Coverage::decoded_window_samples in
  redaction_jpeg_coverage.rs. Preserve existing full-grid exports exactly.
- [x] Extend redaction_jpeg_decoder_tests.rs to export actual cropped geometry.
  Compare actual compressed-image MuPDF tiles and matrices, exercise cache
  fallback, and reject a wrong full-grid mapping control.
- [x] Run the standalone harness and serial native redaction tests. Compare
  prior artifacts, then bind evidence and remaining limits in a checkpoint.

Artifacts: artifacts/jpeg-crop-support-20261102. Runtime probes use only owned
JPEG fixtures and never overwrite prior outputs. No public-admission changes.
