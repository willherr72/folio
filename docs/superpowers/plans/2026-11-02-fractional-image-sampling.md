# Fractional Image Sampling Implementation Plan

> Execute inline with superpowers:executing-plans under the standing phase3
> review boundary and autonomous authorization.

**Goal:** Bound affected native scaled pixels for fractional/cropped placement.

**Architecture:** Child module redaction_render_scale.rs of RendererTransform
uses the existing binary32 interval primitive. Export mupdf_scale_bounds with
SampleRegions input and explicit origin/extent intervals and clip.

**Tech stack:** Rust, existing serial native/harness gates, PyMuPDF probes.

**Spec:** docs/superpowers/specs/2026-11-02-fractional-image-sampling-design.md

## Constraints and review focus

Keep private admission and accepted package unchanged. Tests cover negative
phase asymmetry, single-pixel special paths, edge fallback, output clipping and
invalid interval bounds. No per-helper delegate or commit; final phase3 review
remains pending.

- [x] Add failing tests and API stub in redaction_render_scale.rs and
  redaction_render_scale_tests.rs. Observe expected failures through the fast
  harness. Pin identity/interior support, flips, clipping and invalid input.
- [x] Implement interval division and scale-grid mapping; use binary searches
  over monotone candidate endpoints, explicit edge fallback and sign hulls.
  Run the focused harness and export owned sample/grid bounds for auditing.
- [x] Compare native fz_scale_pixmap output differences against bounds for
  source/edited fixtures across fractions, flips, clips and interval samples.
  Run serial ordinary redaction regression and compare all preceding exports.
  Record checkpoint and remaining renderer/page integration requirements.
