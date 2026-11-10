# Automation paragraph baseline correction

> **For agentic workers:** Use superpowers:executing-plans inline. This correction
> shares the portable milestone's one final review and one correction pass.

**Goal:** reuse fitted paragraph areas without accidental baseline movement while
providing explicit automation control over exact text positioning.
**Architecture:** separate the optional baseline point from the layout rectangle
at the CLI/MCP boundary; use existing native layout and publication checks.
**Tech Stack:** Rust/Serde, CLI/MCP JSON, native integration tests and Python audits.
**Spec:** docs/superpowers/specs/2026-10-29-automation-paragraph-baseline-design.md

## Constraints and review focus

Keep the active full-native run's product/test inputs frozen until it terminates.
One locked/offline Cargo, jobs1, shared target. Preserve every failed attempt and
prior fingerprint. No dependency, renderer, desktop or native layout changes.
No commits, package overwrite, public release or parity claim.

Review exact-input no-ops with an explicit point, area-only legacy clients, nested
projections, rotated/UserUnit coordinates and malformed/nonfinite anchors. Require
runtime geometry/pixel evidence rather than accepting a successful save alone.

## Task: correct and verify the automation boundary

Files:
- `src-tauri/src/automation/paragraph_preview.rs`: strict baseline input, defaulting,
  no-op classification and optional returned control.
- `src-tauri/src/mcp/protocol.rs`: advertise the same strict optional input.
- `docs/automation.md`: explain area versus baseline and fitted-control reuse.
- `src-tauri/tests/automation_fonts.rs`, `paragraph_automation.rs`, `mcp.rs`,
  `nested_paragraph_automation.rs`, `nested_text_automation.rs`: runtime regressions
  and equivalent direct-native comparison inputs.

Interface: `ParagraphEdit.baseline_anchor: Option<BaselineAnchor>` and matching
optional `ParagraphMetadata.baseline_anchor`; strict `{x:f32,y:f32}` in displayed
physical points, finite and absolute value at most 1,000,000. Omitted bounds retain
the source anchor. Bounds with changed dimensions retain it; an equal-size area
move translates it by the origin offset. An explicit anchor is used exactly. Exact source no-ops retain the source point and PDF bytes.
The baseline axis positions the line; frame/alignment determine horizontal placement.

- [x] Reproduce the existing font Fit/area-echo failure with the current CLI and
      immutable authored source. Preserve `font-fit-diagnosis-1` and
      `font-fit-correction/contract-red-1` under the portable artifact directory.
- [x] Trace the second full-gate failure to different native/CLI baseline inputs.
      Preserve both logged images and diagnosis; retain exact image comparison.
- [x] After full-native-2 terminates, inspect every failure and all input checks.
      Check staged baseline hashes and install the five staged test files.
- [x] Run `font-fit-correction/run-focused.ps1 -Phase red`. Require the three new
      baseline tests to fail at runtime, no compiler errors or unexpected failures,
      unchanged input checks, and `outcomeVerified:true` for this expected-red phase.
- [x] Install the two staged runtime files and automation documentation. Use the
      explicit anchor if provided; otherwise retain the source baseline for omitted or
      resized bounds, and translate it only for equal-size area moves. Return the resolved
      anchor when present. A changed anchor must prevent no-op classification.
- [x] Preserve the first focused failure and independent `contract-red-2` showing
      both default-move and default-resize drift. Add the native resize regression
      and correct defaulting without weakening the existing movement assertion.
- [x] Run `run-focused.ps1 -Phase green -Attempt 2`: all five integration targets and all three
      existing paragraph no-op unit tests pass with unchanged inputs. Require the
      expected test identities and `outcomeVerified:true`; diagnose any new failure.
- [x] Run `check-contract.py` against the corrected CLI with a fresh output folder.
      Require exact anchored Fit/echo raster equality, independent 150/160-point
      baseline checks, unchanged neighboring geometry and second-page pixels,
      second-reader text, invalid-input cleanup and original-byte preservation.
- [x] Preserve old root fingerprints/extra-input inventories and prepared consumers.
      Expand the declared source delta to exactly five version files and two runtime
      files; update only the five declared test hashes in extra inputs. Bind corrected
      documentation and the unchanged three example forwards explicitly.
- [x] Update the packaged MCP area-resize caller to pass its inspected baseline
      explicitly; preserve its prior helper and adaptation hashes.
- [x] Prepare fresh full-native-3 and verification-pipeline-3; rebind package consumers
      before continuing the portable plan. Original Word/producer controls are still
      required. No optimized build until corrected verification is complete.

Prepared copies are implementation candidates, not passing evidence. The ordinary
native suite and actual extracted-package acceptance remain required by the parent
portable plan after focused verification succeeds.
