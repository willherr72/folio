# JPEG Tile Placement Implementation Plan

> Execute inline using superpowers:executing-plans under standing authorization
> and the existing phase3 final-review boundary.

**Goal:** Compose actual JPEG crop frames with bounded native placement/scaling.

**Architecture:** RendererTransform tile-frame method; shared interval division;
owned decoder test export connecting crop cell bounds to placement and scaling.

**Tech stack:** Rust, existing serial native harness, native PyMuPDF audit.

**Spec:** docs/superpowers/specs/2026-11-02-jpeg-tile-placement-design.md

## Constraints and review focus

Preserve source-removal identity, private admission and accepted packages. Test
actual-versus-requested cache frames, noncommutative affine composition, partial
edge rounding, full-frame identity and malformed dimensions. No helper review
or commit; final integrated phase3 review stays pending.

- [x] Add a failing tile-frame API and native arithmetic regressions in
  redaction_render_tile_tests.rs. Observe red in the existing fast harness.
- [x] Move proved division onto Interval; implement validated fractional frame
  precomposition. Export linked tile/scale bounds from the existing decoder
  corpus without changing previous exports. Verify fast green.
- [x] Audit fresh/cached native JPEG transforms and scaled before/after pixels;
  run ordinary native redaction regression, compare previous outputs and pin
  final evidence. Record remaining device/affine/paint integration work.
