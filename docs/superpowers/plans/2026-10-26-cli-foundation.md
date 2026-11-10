# Folio CLI Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline.

**Goal:** Ship a locally testable console CLI for inspection, faithful text export
and PNG rendering, with a shared native boundary for later MCP.

**Architecture:** Typed one-shot automation requests own a PdfEngine document guard.
The console parses arguments and formats versioned JSON; existing exporters retain
permission, geometry, output and resource rules. No new dependency.

**Tech Stack:** Rust std, serde_json, tempfile, existing Tauri engine and PDFium.

**Spec:** docs/superpowers/specs/2026-10-26-cli-foundation-design.md

## Global constraints

10,000 pages; 1-based CLI pages; PNG DPI72/150/300/600; password1024UTF-8bytes;
bounded stdin1027bytes; schemaVersion1; exits0/2/1; no overwrite; native paths.
One Cargo/build, shared target, one job. Preserve all work/apps/inputs. No commits,
cleanup, new dependencies or releases. Existing desktop binary remains default-run.

## Review focus

1. No-clobber publication must survive a destination appearing after preflight (Task2).
2. Export must include imported annotations/physical geometry and enforce copy rights (Task2).
3. Non-UTF8 option values, repeated or irrelevant flags cannot silently alter intent (Task1).
4. Password errors must not echo stdin; overlong input bounded before loading engine (Task1/2).
5. Native document handles and registered resources are released on failure (Task2).

### Task 1: Console contract

Files: add src-tauri/src/bin/folio-cli.rs, src-tauri/src/cli.rs;
modify Cargo.toml (binary/default-run) and lib.rs; add tests/cli.rs.

Interface: `cli::run(args: impl IntoIterator<Item=OsString>, stdin: impl Read,
stdout: impl Write, stderr: impl Write) -> i32`. Parsing produces typed request,
engine path and password-input choice. Shared automation request follows Task2.

- [x] Add subprocess tests for absent command/help/version, strict arguments and
  password bounds; run cargo test --test cli, observe missing CLI failure.
- [x] Implement parser, help/version and error envelopes; no GUI launch. Keep PDF
  work behind shared typed dispatch. Add parser unit tests for platform paths.
- [x] Run focused tests; terminal result and exact limitations in ledger.

### Task 2: Shared native operations

Files: add automation.rs and focused tests, extend CLI integration fixtures.
Interface: `automation::execute(&PdfEngine, Request, Option<&str>) -> EngineResult<Value>`;
Request owns input and command-specific output/page/DPI settings. `cli::run` calls it.

- [x] Add failing native/subprocess tests for info, text, PNG and permissions;
  inspect failures before implementation.
- [x] Implement source RAII guard, PagePlans retaining imported overlays, shared
  desktop export calls, text staging/no-clobber publication. No raw extraction.
- [x] Test byte/pixel equivalence, Unicode names, source/output hardlinks,
  destination race, protected inputs, UserUnit/rotation, malformed file, cleanup.
- [x] Run lib + CLI + text_export + page_image_export affected targets; inspect
  terminal results. Fix by observed RED/GREEN, not weakening assertions.

### Task 3: Packaging and independent review

Files: docs/automation.md; evidence under artifacts/cli-foundation-20261026.

- [x] Stage new CLI alongside renderer/notices without overwriting earlier builds.
  Exercise packaged commands from a directory outside the checkout, then verify
  output using independent readers and original hashes.
- [x] One fresh-context final implementation review; fix demonstrated findings
  with RED/GREEN and appropriate affected suite. Preserve coverage limitations.
- [x] Record exact hashes/results, update roadmap and continuation. CLI milestone
  complete only after all required checks; overall product goal remains incomplete.

## Completion evidence

Final native529passed/4ignored;10actual CLI subprocess regressions included.
ReviewP2 native-path JSON failure reproduced and corrected; response records limits.
Corrected portable-cli-2 and packaged-run-2 verified. Execution ledger and product
quality loop contain exact hashes and paths. No MCP/release/parity claim.
