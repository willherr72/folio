# Stable Text Inspection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Expose exact existing text and stable selections through CLI and MCP.
**Architecture:** A shared automation inspection module wraps native complete-source
inspection. A SHA256 selection is regenerated from loaded source/page/run data;
MCP invokes the same CLI in its existing bounded disposable worker.
**Tech Stack:** Existing Rust, serde_json, sha2, native PDFium, stdio MCP.
**Spec:** `docs/superpowers/specs/2026-10-27-text-inspection-design.md`

## Global Constraints

Page 1..10000; offset 0..100000; 100 runs per batch; 16 KiB serialized run;
2 MiB result. Copy and edit permissions required. No new dependencies, persistent
inspection outputs, native policy changes, commits, cleanup or parity claims.
Preserve existing worktree and all prior inputs/apps/evidence. One Cargo at a time.

## Review Focus

- Duplicate words on different pages must not collide (task 1 identity test).
- Generated spaces must retain both extracted and operand text (task 1 parity).
- Edit permission without copy permission must not expose text (task 1 rights).
- Unsupported and empty pages must not appear as successful edit promises (task 1).
- Restarted workers and changed original paths must retain snapshot identity (task 2).

### Task 1: Native-backed CLI inspection

Files: create `src-tauri/src/automation/inspection.rs`; modify
`src-tauri/src/automation.rs`, `src-tauri/src/cli.rs`; create
`src-tauri/tests/text_inspection.rs`.
Consumes: `PdfEngine::list_occurrence_text_runs`, `source_bytes`, DocumentInfo.
Produces: `inspection::inspect(engine: &PdfEngine, info: &DocumentInfo,
page: u32, offset: usize) -> EngineResult<Value>` and Request::InspectText.

- [x] Write real subprocess tests for the spec's CLI cases, comparing native runs
  field-for-field after removing selectionId. Observe unknown-command failures.
- [x] Implement bounded serialization, stable selectors, permission checks,
  pagination and required page/optional offset parsing. Preserve existing CLI paths.
- [x] Verify focused `cargo test --locked --offline --manifest-path src-tauri/Cargo.toml
  --test text_inspection --test cli -- --test-threads=1`; record actual pass counts.

### Task 2: MCP worker integration and packaged acceptance

Files: modify `src-tauri/src/mcp/protocol.rs`, `src-tauri/src/mcp/worker.rs`,
`src-tauri/tests/mcp.rs`, `docs/automation.md`; add milestone artifacts/ledger.
Consumes: CLI inspect-text output from task 1; no ephemeral source IDs.
Produces: folio_inspect_text tool and packaged CLI with exact patched renderer.

- [x] Add failing actual MCP tests for tool discovery, native result parity,
  repeated worker stability, source changes, no folders, strict page/offset/path
  arguments. Update expected tool count to five. Observe failures.
- [x] Implement Call::InspectText and a read-only worker kind with private
  temporary operation storage; tool returns CLI result unchanged.
- [x] Run focused MCP suite. Document commands, bounds, preliminary eligibility,
  permissions and limitations in automation guide.
- [x] Fresh scoped review of frozen baseline/delta; fix demonstrated correctness
  findings with failing regressions and a green suite.
- [x] Run library + CLI + inspection + MCP + relevant native text suites with
  before/after product fingerprints. Build a new portable CLI directory; run actual
  CLI and official SDK modern/legacy checks on public PDFs outside the checkout.
- [x] Record verified checkpoint and exact next preview/save work. Preserve all
  failed logs and prior executables. No redundant second reviewer or broad reruns.
