# Combined Portable Implementation Plan

> Use superpowers:executing-plans inline, with one fresh final scoped review.

**Goal:** one verified local desktop/CLI portable development checkpoint.
**Architecture:** extend existing PowerShell release assembly and Python integrity
verifier; both binaries share the current native engine and packaged renderer.
**Tech Stack:** existing PowerShell, Python unittest, Rust/Tauri, npm/Vitest.
**Spec:** docs/superpowers/specs/2026-10-27-combined-portable-design.md

## Global constraints
Version 0.22.0-dev.1, no overwrite of older packages, no commits/push/release/cleanup.
One Cargo, locked/offline, jobs1, established shared target. No product feature
changes or dependencies. Preserve originals, artifacts and user's running apps.

## Review focus
- Missing/stale CLI must fail verification even if desktop and ZIP are present.
- ZIP duplicates/mismatched entries must fail; verifier never extracts archive paths.
- Wrong source fingerprint or changed packaged renderer must fail.
- Running verification outside checkout must use explicit package/archive inputs.
- Existing output package/report must remain intact on a repeated attempt.

## Task 1: testable combined package integrity
Files: scripts/verify-portable.py; scripts/tests/test_verify_portable.py.
Interface: verify_package(root: Path, archive: Path, expected_version=None) -> dict;
CLI --package PATH --archive PATH [--report NEW_PATH], defaults to current package.
- [x] Write real temporary ZIP/package tests for valid package and each Review Focus
case; run `python -m unittest discover -s scripts/tests -p test_verify_portable.py -v`
and retain the missing-interface RED.
- [x] Refactor verifier under main guard, implement both binary hashes, required
guide, manifest consistency, ZIP equality and existing OCR/license checks.
- [x] Run full Python script test discovery; require zero failures.

## Task 2: matching release assembly and documentation
Files: scripts/build-portable.ps1, scripts/collect-licenses.ps1, package.json,
package-lock.json, src-tauri/Cargo.toml, Cargo.lock, tauri.conf.json, README.md,
CHANGELOG.md, docs/automation.md.
- [x] Preserve baseline before edits. Bump only root package version to 0.22.0-dev.1.
- [x] Explicit locked/offline CLI release build before final fingerprint check;
copy CLI and record cliExecutableSha256. Keep same renderer/staging/no-overwrite.
Use offline license metadata. Update quick-start and historical checkpoint labels.
- [x] Run frontend suite and current native library/CLI/MCP/forms/text/paragraph
gate serially for Cargo; keep terminal logs and source fingerprints.

## Task 3: runtime package and sole review
- [x] Build the new combined portable package with the shared Cargo target. Verify
archive integrity; extract to a new owned directory outside checkout and reverify.
- [x] Run extracted CLI help/info and SDK consecutive text/paragraph/form workflows,
independent output checks and source hashes. Use the current acceptance scripts
with explicit executable/output overrides, preserving old evidence.
- [x] Freeze scoped baseline/delta/hashes and delegate the sole final review. Address
demonstrated Important/Critical findings with RED/GREEN and affected reruns.
- [x] Smoke-test desktop launch/read with the documented owned-process workflow;
record limits if unavailable. Write checkpoint, disposition and quality-loop update.

Completed local acceptance is recorded in artifacts/combined-portable-20261027/checkpoint.json.
The overall Adobe/Foxit usefulness goal remains active; no release or parity claim.
