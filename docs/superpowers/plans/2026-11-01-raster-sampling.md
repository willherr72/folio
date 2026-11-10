# Raster sampling implementation plan

> Execute inline with superpowers:executing-plans; one final scoped review.

**Goal:** Preserve displayed text clarity while progressing the reusable renderer.
**Architecture:** Preserve existing SVG rendering; after sampling alternatives failed,
verify an app-owned32MiB browser discardable-cache policy. Keep PageRaster unused.
**Tech stack:** TypeScript/React, Playwright/Chromium, Pillow, Vitest.
**Spec:** docs/superpowers/specs/2026-11-01-raster-sampling-design.md

### Task1: Isolate sampling and placement
- [x] Verify prior checkpoint and reproduce with a changed discriminator.
- [x] Compare scale selection, measured canvas geometry and pixel snapping;
  capture inputs/helpers/results and preserve failed hypotheses.
- [x] Specify the correction supported by source and runtime evidence.

### Task2: Implement and verify the correction
- [x] Observe default/64MiB resource failures and32MiB candidate runs against the
  unchanged six budgets; retain every failure.
- [x] Build the product configuration; verify app-owned runtime arguments without
  environment-injected cache flags, then constant/varied63-cycle runs.
- [x] Exercise source/edited/opaque/alpha, crop/rotation/zoom matrix alternatives;
  preserve their exact-display failures.
- [x] Independently reconcile current-binary resources, native pixels, fitting
  state and displayed frames; no silent tolerance changes.
- [x] If the architecture cannot yet satisfy display fidelity, retain that
  blocker and document the tested alternative and next concrete correction.

### Task3: Review and continuation
- [x] One fresh final review; correct consequential defects with regression tests.
- [x] Record exact acceptance and remaining gates; keep the overall goal active.
