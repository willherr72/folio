# Automation Chaining Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkboxes.

**Goal:** Consecutive faithful PDF edits within one MCP session, with owned immutable versions.
**Architecture:** Apply copies a validated preview using the existing supervised CLI;
session adopts the private result as a new document ID. Close frees storage.
**Tech Stack:** Existing Rust/tempfile/serde/sha2/Windows handles and stdio MCP; no dependencies.
**Spec:** docs/superpowers/specs/2026-10-27-automation-chaining-design.md

## Global Constraints

64 live documents/512MiB total; monotonic IDs, no reuse or automatic eviction.
8 previews/512MiB/15min unchanged. All large file proof/copy work stays in workers.
Never publish decrypted source bytes, overwrite originals, hardlink saved files,
accept model paths, or delete saved outputs. One Cargo at a time, locked/offline.
Preserve existing isolated dirty worktree; no commits, releases or desktop restarts.

## Review Focus

- Closing an early document must not redirect remaining IDs to a different source.
- Cancellation/adoption must leave no unauthorized input or partial public output.
- Expiry/close must not remove files while active workers consume them.
- Public saved-file changes must not mutate an applied private source.
- Capacity failures must preserve previews and permit progress after explicit close.

### Task1: Supervisor ownership and recorded evidence gaps

Create src-tauri/src/mcp/session.rs; modify mod.rs/worker.rs/previews.rs and
tests/text_automation.rs. Session owns worker/previews/config and implements
dispatch(Value), poll(), prune(); replace inline stdio dispatch with these methods.

- [x] Add actual MCP Apply/Close flow tests (missing tools must fail first).
- [x] Add second-identical-run CLI save/reopen regression and run it before changes.
- [x] Extract supervisor without behavior changes; add deterministic owned-process
  cancel/busy/expiry/EOF tests for preview/save stages. No production test hooks.
- [x] Run focused lifecycle and duplicate-run tests; existing tests must remain green.

### Task2: Owned document versions and Apply/Close tools

Modify config.rs, previews.rs, protocol.rs, worker.rs, session.rs, tests/mcp.rs.
Config adds can_add(bytes), adopt(directory,bytes,hash)->Value, close(index)->Value;
actual live document ID lookup. Preview exposes verified candidate proof. Worker
Kind::Apply calls existing save-text-preview into a private folder, then adopts it.

- [x] Implement64/512MiB admission and monotonic IDs with deterministic capacity,
  release and ownership tests. Windows retained-file handle deletes on close.
- [x] Add closed-schema Apply/Close tools (10 total); route busy/unknown/capacity
  errors and preserve original source/preview independence.
- [x] Pass actual two-edit workflow, public-file mutation, repeated apply/close,
  unknown IDs and normal cleanup tests. Update guide and ledger.

### Task3: Review and packaged acceptance

Artifacts artifacts/automation-chaining-20261027; baseline/snapshot/hash review.

- [x] Freeze scoped changes and obtain one independent final review. Correct
  demonstrated important defects with failing regressions; no second review loop.
- [x] Run full library/CLI/MCP/inspection/automation gate and fingerprints; build
  fresh portable executable only after success. No unrelated native policy changes.
- [x] Official SDK legacy/modern two-edit Word workflow outside checkout; verify
  final saved bytes and independent reader text/pixels/bookmark/note preservation.
- [x] Record checkpoint, rulings, exact evidence and next workflow gaps. Goal active.
