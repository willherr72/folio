# Integrated Editing Session Audit Implementation Plan

> Use superpowers:executing-plans inline, with one final independent scoped review.

**Goal:** establish repeatable evidence for everyday paragraph editing and resource release.
**Architecture:** coordinator-owned audit harness around the accepted desktop package;
preserve product source until a demonstrated defect requires correction.
**Tech Stack:** existing Node/Playwright CDP, PowerShell process measurements, Python hashes.
**Spec:** docs/superpowers/specs/2026-10-27-editing-session-audit-design.md

## Global constraints
New owned desktop process/profile and copied PDFs only. No commits, cleanup,
overwrites of earlier evidence/packages, release or parity claims. No forced GC.
900x600 is a CSS viewport test, not physical hardware acceptance. One Cargo at a
time if a product correction requires rebuilding; preserve the accepted package.

## Review focus
- Auto-scrolling test clicks must not conceal inaccessible narrow-window controls.
- Preview cancellation and undo must restore the correct document, including after tab switches.
- Instrumentation must retain metadata only and start before any document URLs exist.
- All-tabs-closed observations must allow pending native/render cleanup to finish.
- Process memory alone must not be presented as proof of a leak or of its absence.

## Task 1: capture the integrated baseline
Files: artifacts/editing-session-audit-20261027/audit.mjs and evidence under run-1.
Consumes: accepted combined checkpoint's executable/hash, copied quiz, native-dialog
and process-memory helpers. Produces: exclusive audit.json or failure.json,
incremental samples.json, screenshots and source hashes.
- [x] Verify accepted binary/source identity, write baseline manifest, and launch owned instance.
- [x] Implement the two-viewport workflow and metadata-only URL tracking in the spec.
- [x] Run it, inspect every result and screenshot, and record failures without relaxing assertions.

## Task 2: diagnose and correct demonstrated defects, if any
Files: exact product/test paths determined by Task 1 evidence, recorded before editing.
Consumes: reproducible baseline failure. Produces: root-cause record, failing then
passing regression, relevant suite results and corrected owned runtime evidence.
- [x] Classify failures as product, harness, or unsupported measurement; preserve evidence.
- [x] For product defects, use systematic debugging/TDD, make the smallest sound correction,
run affected frontend/native checks and fresh build as necessary, then repeat the failing workflow.
- [x] If baseline has no defect, record the observed limits; do not invent a product change.

## Task 3: review and record
Files: artifacts/editing-session-audit-review-20261027/review.md,
artifacts/editing-session-audit-20261027/checkpoint.json, docs/product-quality-loop.md.
- [x] Freeze scope/evidence hashes and request the sole final independent review.
- [x] Address demonstrated important findings with appropriate regressions and affected reruns.
- [x] Verify owned-process shutdown, unchanged originals/package, record measurements and limits,
and update the persistent quality checklist without claiming commercial parity.

## Acceptance record

All three scoped audit tasks are complete. The Important review finding was in
the raster oracle, corrected with four failing then four passing controls and
40 corrected runtime cycles (runs 3/4). Runs 1/2 are preserved and qualified.
See artifacts/editing-session-audit-20261027/checkpoint.json for hashes, original
and package preservation, verified root shutdown and the retained limits. No
product correction was justified by this audit.

The independently added full ordinary native regression remains pending in
artifacts/native-regression-20261027 (same session 91716). It is not a passing
gate or a prerequisite inferred to have passed from the desktop audit. Keep
the broader verification plan in progress until its terminal result is inspected.
