# Unsaved Tab Isolation Audit Implementation Plan

> Use superpowers:executing-plans inline, with one final independent scoped review.

**Goal:** verify that native paragraph edits, undo histories and saves retain their document ownership across tab switches.
**Architecture:** coordinator-owned audit against the unchanged accepted desktop; make product changes only for a reproduced defect.
**Tech Stack:** Node/Playwright CDP, PowerShell native-dialog helpers, Python independent PDF readers.
**Spec:** docs/superpowers/specs/2026-10-27-dirty-tab-audit-design.md

## Global constraints

Original native session 91716 failed on a test fixture; its focused correction passed.
Retry session 80037 may run alongside the isolated desktop audit because product
sources and the accepted executable remain unchanged. Require both terminal
acceptances before the final checkpoint; do not change product code mid-run.
Use an owned process/profile and exclusive copies/outputs under
artifacts/dirty-tab-audit-20261027. Preserve originals, old evidence, accepted
packages and user processes. No commits, publication or parity claim. CSS viewport
tests are not physical hardware acceptance. One Cargo at a time if rebuilding.

## Review focus

- Identical source text can conceal wrong-tab ownership: assert distinct tab/page IDs.
- Missing/stale image payloads can falsely pass: capture the successful valid digest.
- Inactive dirty-tab close must name that tab and cancellation must retain both edits.
- Saving A must not mark B clean or publish B's different paragraph.
- Viewport/render changes can alter image hashes: record baselines after settling each viewport.

## Task 1: native ownership workflow

Files: create artifacts/dirty-tab-audit-20261027/prepare.py, audit.mjs,
save-native-dialog.ps1 and save-dialog-guards.py;
results go in exclusive run directories. Read existing raster-observation.mjs,
start-release-test.ps1, set-native-dialog.ps1, set-release-dialog.ps1 and
stop-release-test.ps1 unchanged. The audit-local Save wrapper adds owned-path guards.
Consumes: accepted package hash, original input metadata from the prior audit,
and process.json from the owned launch. Produces: baseline.json, incremental
observations.json, exclusive audit.json or failure.json, screenshots and two saved PDFs.

- [x] Inspect the first terminal native result and diagnose/correct its fixture failure;
  bind the focused green result and unchanged product hashes before desktop launch.
- [x] Create and hash exclusive Quiz A.pdf/Quiz B.pdf copies and verify the accepted binary.
- [x] Implement the six spec workflows at both viewports, including exact named-tab
  dirty indicators and captured baseline/applied/Undo/Redo observations.
- [x] Run the owned workflow; inspect every assertion, saved path and screenshot.
  If a harness failure occurs, preserve its run and make the correction explicit.

## Task 2: saved output and preservation

Files: create artifacts/dirty-tab-audit-20261027/verify.py and evidence reports.
Consumes: successful audit.json, original inputs and saved A outputs. Reuse the
fixture-specific audit function in artifacts/combined-portable-20261027/accept-quiz.py
without changing that prior oracle. Produces: independent.json with per-output hashes,
text/style/outside-area pixel results and the oracle's explicit limits.

- [x] Verify both saved A outputs against the expected A paragraph, source font/size/color
  and padded outside-area pixels; verify each saved hash and source/copy hash.
- [x] Reverify the accepted package/archive, then stop the owned root by recorded identity.
- [x] For any demonstrated product defect, record exact source/test paths, reproduce
  it in a failing regression, make the smallest correction and run the affected
  suites/build plus corrected desktop workflow before proceeding.

## Task 3: review and durable checkpoint

Files: artifacts/dirty-tab-audit-review-20261027/review.md and response.md,
artifacts/dirty-tab-audit-20261027/checkpoint.json, docs/product-quality-loop.md.

- [x] Freeze source/harness/evidence hashes and request the sole scoped review.
- [x] Address important demonstrated findings with appropriate failing controls and
  affected runtime reruns, retaining the original report and all failed evidence.
- [ ] Inspect retry session 80037's terminal native and fingerprint results; resolve
  any failures before recording the final accepted checkpoint.
- [ ] Record exact acceptance, input/package preservation, shutdown and remaining
  gaps in the checkpoint and quality checklist. Leave the overall goal active.

Plan self-check: each of the six spec workflows is owned by Task 1; saved output
and preservation are Task 2; review/disposition is Task 3. The five review risks
are explicit assertions in Tasks 1/2. Runtime and full-native acceptance remain
pending; preparation and concurrency do not establish either result.

Acceptance so far: corrected run-4 passed both viewports and 46 recorded
observations. Both saved A PDFs passed independent readers, style and padded
outside-area pixel checks, including stale-source rejection. Package/original
preservation and owned shutdown verified. Runs 1–3 remain failed harness evidence;
no product defect was demonstrated. Full native retry and final review remain pending.

Post-review: run-5 closes the P3 evidence gap and adds post-cancellation A history and exact B paragraph checks. Two viewports/58 observations, independent saved-output checks, package preservation and owned shutdown pass. Native retry80037 has unresolved failures; final acceptance remains pending. See checkpoint-native-unresolved.json and the sole review response.
