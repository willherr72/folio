# Validated Text Automation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Track steps using checkboxes.

**Goal:** Complete inspectâ†’previewâ†’new-copy save for existing PDF text via CLI/MCP.
**Architecture:** Revalidate stable selectors and run the native complete-source
transaction in a disposable CLI worker. Retain hash-bound preview bundles; publish
their exact candidate bytes with exclusive file creation. MCP owns preview IDs.
**Tech Stack:** Existing Rust/serde/sha2/tempfile/native engine; current stdio MCP.
**Spec:** docs/superpowers/specs/2026-10-27-text-automation-design.md

## Global Constraints

No new dependencies/native policy relaxations.32KiB edit JSON;4096UTF8 replacement;
64KiB manifest;512MiB PDF/bundle;32MiB each PNG;8MCP previews/512MiB total/15minutes.
One Cargo/build at a time. Preserve dirty worktree, originals and artifacts; no
commits, releases, destructive cleanup or permission loops.

## Review Focus

- Source changed or duplicate visible words: resolve only the regenerated selector.
- Protected PDFs: save encrypted bytes, never decrypted engine source_bytes.
- Output/candidate/manifest tampering: refuse publication and preserve existing files.
- Preview ownership under cancel, capacity and expiry: no orphan candidate or public output.
- Source-linked annotations/bookmarks and other pages: preserve full-source graph.

### Task 1: Native CLI preview bundles and exact copy saving

Create automation/text_preview.rs and tests/text_automation.rs. Modify
automation/inspection.rs (reusable selector resolver), automation.rs and cli.rs.
Interfaces: strict TextEdit and PreviewManifest; preview(engine,info,edit,parent,
password)->Value; save(bundle,manifest_hash,output)->Value; validate_bundle uses
fixed file names and verifies streamed bounded file hashes. Paths stay native.

- [x] Add subprocess regressions for full flow, stale/refused edits, protected
  documents, source deletion, exact copies, integrity and exclusive destinations.
  Watch new-command failures before implementation.
- [x] Implement shared selector resolution and native immutable transaction,
  protected export/reopen/render bundle, strict manifest and streaming copy save.
- [x] Run text_automation, text_inspection and cli targets; verify terminal output.

### Task 2: MCP preview ownership and lifecycle

Create mcp/previews.rs; modify mcp/{mod,protocol,worker}.rs and tests/mcp.rs.
Interfaces: PreviewStore owns TempDir-backed Preview records and their immutable
manifest hashes, capacity and expiry. Worker takes/returns owned preview on
successful native preview; Save starts from a store record, never model paths.

- [x] Add failing actual MCP workflow and strict arguments/ownership regressions;
  update discovery count to8. Add targeted deterministic expiry/capacity coverage.
- [x] Implement preview/save/discard calls, bounded adoption, prune only when idle,
  worker-before-preview cleanup and unchanged worker supervision guarantees.
- [x] Run MCP and library lifecycle coverage; update automation guide.

### Task 3: Review, package and acceptance

Artifacts under artifacts/text-automation-20261027 and review sibling; ledger here.

- [x] Freeze scoped baseline/delta and obtain one fresh independent review. Fix
  demonstrated defects with REDâ†’GREEN regressions; preserve report and failed logs.
- [x] Run focused library/CLI/MCP/inspection/automation tests plus applicable native
  text/publication regressions; fingerprint before/after. Build a fresh portable CLI.
- [x] Verify public Word insert/save/reopen using official SDK legacy/modern from
  external cwd; independent reader checks text and untouched page pixels/graph.
- [x] Record exact evidence/limitations and next workflow gaps; keep overall goal active.
