# Folio MCP Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans.

**Goal:** Usable local MCP inspection/export over Folio's validated CLI engine.
**Architecture:** Bounded Rust stdio dispatcher, launch-authorized snapshots,
one owned CLI subprocess per PDF call, isolated export directories.
**Tech Stack:** Existing Rust std, serde_json, tempfile; no new dependencies.
**Spec:** docs/superpowers/specs/2026-10-26-mcp-foundation-design.md

## Global Constraints

Preserve all dirty work, inputs, builds, apps and evidence; no commits or cleanup.
One Cargo at a time; existing shared target, CARGO_BUILD_JOBS=1. Limits and exact
protocol/tool names as in spec. All native safety checks remain unchanged.

## Review Focus

1. Requests without IDs cannot execute writes (Task1 notification tests).
2. Path-shaped IDs/extra path arguments cannot widen access (Task2 tests).
3. Failed/cancelled children cannot retain running workers (Task2 lifecycle tests).
4. Legacy state cannot suppress modern per-request validation (Task1 era tests).
5. Existing export/source bytes survive repeated operation errors (Task2 tests).

### Task 1: Bounded protocol and launch configuration

Files: new src-tauri/src/mcp/{mod,protocol,config}.rs and tests/mcp.rs;
modify src-tauri/src/bin/folio-cli.rs, src-tauri/src/lib.rs, CLI help only.
Interfaces: mcp::run(Vec<OsString>)->i32; Config owns snapshots, labels, renderer
and output parent. Protocol::dispatch(Value) returns reply or typed tool call.

- [x] Write subprocess tests for help, discovery, legacy handshake, strict
  modern metadata/version errors, invalid JSON recovery and no-reply notifications.
- [x] Run `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml
  --test mcp -- --test-threads=1`; observe missing MCP command assertions.
- [x] Implement bounded framing, configuration and protocol; four tool schemas.
- [x] Repeat tests; verify green and record log. Preserve baseline copies.

### Task 2: Isolated native operations and lifecycle

Files: new src-tauri/src/mcp/worker.rs; extend mod.rs and tests/mcp.rs.
Interfaces: Worker owns Child + TempDir, starts validated CLI arguments, polls
bounded JSON result, kills/reaps on Drop; operations use snapshot IDs only.

- [x] Add actual PDF info/text/render, snapshot replacement, strict IDs/options,
  malformed/protected PDF, repeated calls, cancellation/EOF tests.
- [x] Observe RED before adding worker behavior.
- [x] Implement one worker at a time,120s deadline, bounded stdout/stderr,
  result mapping and 65536-byte safe UTF-8 excerpt, keep output only on success.
- [x] Run focused suite and record green. Verify original/export sentinels.

### Task 3: Review, regression and portable acceptance

Files: docs/automation.md, product-quality-loop.md; this ledger/artifacts.

- [x] Document launch configuration, tool examples and limits.
- [x] Run native lib + cli + mcp + page_image_export + text_export tests with
  frozen fingerprint. Build portable CLI including renderer license/provenance.
- [x] Fresh scoped review (skill-required); fix important findings RED→GREEN
  and run affected suite. No release or parity approval inferred.
- [x] Outside-checkout portable MCP acceptance: both eras, actual Word PDF
  info/text/render, independent text/pixels/unchanged source verification.
- [x] Record final evidence, hashes and continuation; no commits or cleanup.
