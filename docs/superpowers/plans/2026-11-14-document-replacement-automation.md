# Document Replacement Automation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline, preserving the authorized worktree and prior component authorities. Steps use checkbox syntax.

**Goal:** Provide complete document replacement planning and actual CLI/MCP batch preview consumers.

**Architecture:** Aggregate native page plans within one immutable worker source,
then adapt exact proposed edits to the existing inspected-selection batch preview.
All errors, blockers and limits retain complete-or-error semantics.

**Tech Stack:** Existing Rust/PDFium/lopdf, pinned Unicode matcher, serde, CLI and local MCP.

**Spec:** docs/superpowers/specs/2026-11-14-document-replacement-automation-design.md

## Global constraints

Page-planning correction checkpoint is a prerequisite; do not change source
until its running qualification finishes. No commits, cleanup, package/version/
launcher changes, new dependencies, clipboard or private-input operations.
Serialize native/build/runtime with one Cargo job and existing D: cache/temp.
Limits:10000 pages/matches/selections,4 MiB native document plan,512 KiB requests,
2 MiB final transport envelopes; unchanged page and64 edit/32 changed-page/512 KiB
actual-native-byte batch limits. One final review after both tasks, one Important
correction pass, no re-review. This plan is not desktop or commercial completion.

## Review focus

- Late-page errors must never produce a partial Replace all preview (Task1/2).
- Many scanned pages with few changed pages must respect the correct limits (Task1).
- A different source or changed interpretation must invalidate expectedPlanId (Task2).
- Generated/native identity differences must survive selection conversion (Task2).
- Protected input and cancelled workers must retain permission/ownership boundaries (Task1/2).

### Task 1: complete native document planner

**Files:** new src-tauri/src/document_replacement_plan.rs and
document_replacement_plan_tests.rs; modify engine.rs request/dispatch, lib.rs
exports, replacement_plan.rs visibility only as needed for checked aggregation.

**Interfaces:** expose TextReplacementSelection, TextReplacementDocumentPlan,
ReplacementPlanningControl and ReplacementPlanningProgress. Implement
`PdfEngine::plan_text_replacement_document(source_id,query,replacement,options,
selected,control) -> EngineResult<TextReplacementDocumentPlan>` as specified.
Worker checks actual count/permissions, loops raw page plans, scales per-page
once and performs global selected/budget validation before returning.

- [x] Capture page-component completion/checkpoint/current source/shared inputs
  and exact preimages before changes; preserve all prior authorities.
- [x] Write runtime RED tests named document_replacement_planning_* for last-page
  matches, shared occurrences, cross-object late blocker, excluded blocker,
  invalid pairs,32/33 changed pages,64/65 edits,10000 match/4 MiB bounds, different
  page UserUnits, permissions/no-op, no-source-registration and cancellation
  before start/between pages. Assert no executable child edits on global failure.
- [x] Run focused release-dependency/Folioopt0 library tests with source/shared
  guards and observe intended runtime failures, then implement the interface.
- [x] Run the same focused tests GREEN plus existing page-planner/batch tests.
  Check exact bytes/source counts, complete counts and ordered progress events.

### Task 2: stable automation planning and batch preview

**Files:** new src-tauri/src/automation/replacement.rs; modify automation.rs,
automation/inspection.rs and nested_inspection.rs for shared exact selection
encoding, cli.rs, mcp/protocol.rs, mcp/worker.rs and worker integration as needed.
Add src-tauri/tests/replacement_automation.rs; update docs/automation.md and
docs/page-replacement-planning.md to describe only qualified behavior.

**Interfaces:** strict ReplacementRequest version 1, deterministic plan identity,
plan-replacement/preview-replacement CLI commands and matching MCP tools.
Task1 DTOs are internal authority; client requests provide search intent and
expectedPlanId, never native source paths. Existing BatchEdit, preview manifests,
Save/Apply/Discard and preview adoption remain the output contracts.

- [x] Add parser/protocol/model tests and real-PDF runtime RED for stable reopen
  identity, request strictness, stale ID/source rejection, late failure cleanup,
  generated spaces, root/nested selection conversion and final-envelope overflow.
- [x] Implement read-only report conversion and exact selection encoding reuse.
  Implement preview by replanning/checking expectedPlanId, constructing BatchEdit
  and calling existing preview; reject blocked/no-change results before staging.
- [x] Add CLI/MCP dispatch, cancellation and response-envelope checks. Assert
  MCP readOnlyHint true for planning and false/non-destructive for preview.
- [x] Run focused GREEN, complete library correctness and affected ordinary
  optimized integrations once source is frozen. Qualify unchanged frontend input
  reuse; do not claim a fresh frontend run or GUI interaction.
- [x] Build the normal combined development candidate. Run actual compiled CLI
  and MCP plan->preview->Save/reopen and Apply->replan chains over authored
  multi-page root/shared/protected fixtures. Exercise stale/cancelled/blocked
  requests without partial bundles and preserve original bytes.
- [x] Independently compare saved text, metadata, forms/fonts, links/bookmarks,
  encryption and unselected glyphs/pixels outside fixed authored masks. Verify
  auditor rejection with deliberate saved-copy corruptions.
- [x] Request one final scoped component review, resolve all Important findings
  in one TDD pass, defer Minor coverage explicitly and rule on every declined
  behavior. Verify corrected evidence and checkpoint. No re-review or release.

Self-review: native counts, physical scaling, global blockers/cancellation belong
to Task1; opaque identities, transport limits and usable mutation consumers belong
to Task2. Both tasks include the relevant review-focus witnesses. Native public
indices and automation page numbering are intentionally different at one boundary.
Execution is already authorized inline; no approval menu or new worktree is needed.
