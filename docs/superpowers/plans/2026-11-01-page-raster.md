# Reusable page raster implementation plan

> Execute inline with superpowers:executing-plans. One fresh final scoped review.

**Goal:** Build a bounded, race-safe raster foundation and establish its remaining integration gates.
**Architecture:** Serial owned decoder plus persistent React SVG/canvas component;
the caller owns the URL and geometry. SVG fallback preserves unsupported inputs.
**Tech stack:** TypeScript, React, Vitest, Playwright, existing esbuild, Pillow.
**Spec:** docs/superpowers/specs/2026-11-01-page-raster-design.md

## Global constraints

No PageView switch, package edits, dependencies, commits or cleanup. Preserve
prior strict display failures. Cap encoded PNG at 32 MiB, each axis at 16384,
total pixels at 16,777,216. Only one decode in flight per mounted controller.

## Review focus

Stale A-B-A completion; bitmap/fetch lifetime after unmount; fallback error events;
malformed/oversized encoded data; geometry and transparent compositing.

### Task 1: Bounded ownership and React presentation

Files: src/editor/pageRaster.ts, src/components/PageRaster.tsx,
tests/page-raster.test.tsx. Interface: PageRaster({url?,width,height,transform?,
onStatus?}); status reports url, phase (empty/loading/ready/error), mode (canvas/svg).

- [x] Write failing unit tests for serialized source changes, stale completion,
  unmount, bounded pre-decode PNG admission and URL ownership. Run focused Vitest;
  expected missing implementation failure, then implement and require all pass.
- [x] Add component tests for stable surface, geometry, no stale display,
  fallback success/error/recovery and callback ownership; observe RED then GREEN.

### Task 2: Actual browser and regression evidence

Files: tests/page-raster.browser.mjs, tests/fixtures/page-raster.tsx;
artifacts/page-raster-20261101/ for frozen inputs, scripts, PNGs and results.

- [x] Bundle the component with existing esbuild; use an owned headless Chromium
  process without a server. Assert raw opaque pixels, bounded surface count,
  actual decode readiness, overlay order, pointers and focus. Save paired display
  PNGs across geometry/alpha/native source and edited fixtures; independently
  compare them without treating diagnostic differences as shipping passes.
- [x] Run affected tests, full frontend suite and npm run build; expected zero
  failures. No Cargo/native suite needed for an unconnected frontend component.

### Task 3: Review and checkpoint

- [x] Freeze exact scope and evidence; request one fresh final scoped review.
- [x] Resolve important findings with regression tests; record coverage limits.
- [x] Update roadmap and continuation. Keep editor integration, owned Windows
  acceptance, memory acceptance and overall quality goal open.
