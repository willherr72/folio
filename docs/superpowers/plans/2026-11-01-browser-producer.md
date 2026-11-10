# Browser PDF compatibility plan

> For agentic workers: use superpowers:executing-plans inline.

**Goal:** Expand original-producer compatibility and measure optimized edit stages.
**Architecture:** Immutable owned Edge print outputs, accepted0.29CLI, saved cycles,
fixed-scope independent audits. Refusals stay failures, not accepted outputs.
**Tech stack:** Edge/Playwright, Python/PyMuPDF/pypdf, Folio CLI.
**Spec:** docs/superpowers/specs/2026-11-01-browser-producer-design.md

## Constraints and review focus
No original rewriting or publication. Pin HTML/generator/browser identity, source
PDFs, CLI/renderer and helpers. Close owned browser/CLI. Record unsuccessful cases.
Independent expectations cover text order, neighbors, links, font programs, original
tag semantics where claimed and unchanged second page. Measure CLI cold-process
wall time explicitly; do not equate it to UI/debug timing or universal performance.

## Tasks
- [x] Generate original tagged/untagged two-page PDFs and independent manifest;
  bind current accepted executable and unchanged native/dependency identity.
- [x] Inspect and run three targets × text/paragraph × two sources × two saved
  cycles, recording calls, refusal stage, original/current hashes and timings.
- [x] Independently audit every successful saved output with fixed expectations
  and nonvacuous corruption controls; diagnose any unsupported original behavior.
- [x] Reproduce any actual product defect before changing implementation; if a
  deeper subsystem is required, record a concrete next-stage plan and evidence.
- [x] Fresh scoped review and persistent checkpoint; leave broad quality goal active.
