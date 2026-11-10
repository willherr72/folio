# Renderer Grid Sizing Implementation Plan

> Execute inline with superpowers:executing-plans under the existing phase3
> final review boundary. Preserve inherited changes and accepted packages.

**Goal:** Derive decoder sizing from bounded native renderer placement.

**Architecture:** Child module of RendererTransform, retaining private interval
invariants; connect fitted axis extents to redaction_render_sizing.

**Tech stack:** Rust, existing native harness, PyMuPDF native bindings.

**Spec:** docs/superpowers/specs/2026-11-02-renderer-grid-sizing-design.md

## Constraints and review focus

No raw-source admission change. No copied third-party implementation. Tests must
cover negative fractional translations, quarter-turn tiled origin mapping,
uncertain epsilon branches, square-root rounding boundaries and unsafe integer
casts. One final phase3 review remains pending; no helper review or commit.

## Implementation and evidence

- [x] Add GridFit mode and failing tests in redaction_render_placement_tests.rs:
  identity stays exact; outward [10.2,0,0,-8.3,-0.2,9.4] encloses native snapped
  coefficients; tiled quarter turns and uncertain branches enclose each actual
  result; invalid integer conversions refuse.
- [x] Implement RendererTransform::mupdf_sizing(source, mode) in child module
  redaction_render_placement.rs. Return ImageSizing with fitted transform,
  extent intervals and maximum_reduction from existing mupdf_maximum.
  Run artifact run-harness.ps1 red/green and inspect terminal results.
- [x] Export owned cases and compare every coefficient and extent with actual
  fz_gridfit_matrix / fz_get_pixmap_from_image, including native decode results.
  Run serial ordinary redaction regressions, compare predecessor outputs and
  checkpoint final source, scripts, fixtures, runtime and evidence hashes.
