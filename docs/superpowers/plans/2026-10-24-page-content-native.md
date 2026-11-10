# Native page content replacement implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan inline, task by task. Steps use checkbox syntax for tracking.

**Goal:** Provide an immutable native page-content replacement that retains destination interactions and page identity.

**Architecture:** Import content/resources into the original page dictionary with a fitted transform. Isolate retained appearance resource contexts, prune obsolete graphs, then register a derived source with destination policy. Keep UI integration a separate milestone with preview and undo acceptance.

**Tech Stack:** Rust, lopdf, PDFium, existing worker queue and policy.

**Spec:** docs/superpowers/specs/2026-10-24-page-content-replacement-design.md

## Global constraints

- No original file writes, commits, release, or cleanup of prior checkpoints.
- Preserve destination geometry, annotations, forms, bookmarks, policy and other pages.
- Incoming protected sources and Folio overlays are unsupported initially.
- Keep existing text-edit restrictions; do not claim all imported text editable.
- Bound import resources to existing 64-depth / 1M-values / 512 MiB limits, decoded page content to 16 MiB, final output to 512 MiB.
- Bound annotation appearance/direct-resource copies to cumulative 16 MiB / 100,000 values before allocation; share isolated APs only within the same page context.
- Preserve failed evidence and use fresh output paths for each run.

## Review focus

- Shared annotation appearances must not bind to incoming fonts; test absent AP resources and colliding font names.
- Repeated edits must not retain obsolete graphs; test stable reachable object count.
- Page dictionaries and form hierarchies must retain identity; test fields, bookmarks and unselected pages.
- Restricted sources must not gain rights; test destination denial and incoming protected denial.
- Unequal geometry, rotation and UserUnit must not move retained interactions; test all quarter turns and physical scaling.

### Task 1: Native replacement and fidelity tests

**Files:** create src-tauri/src/page_content_replace.rs and src-tauri/tests/page_content_replace.rs; modify engine.rs and page_resources.rs for worker dispatch and bounded resource reuse. Use the existing open_bytes registration path without modifying native_comments.rs.

**Interfaces:** `PdfEngine::replace_page_content(&self, destination_id: &str, destination_page: usize, incoming_id: &str, incoming_page: usize) -> EngineResult<DocumentInfo>`; returns a full immutable destination source. No IPC yet.

- [x] Write `replaces_content_retaining_interactions_and_editability` and run it RED against the missing API.
- [x] Implement bounded direct import, affine fit, conditional clip, AP isolation, metadata/policy guards and derived-source registration.
- [x] Run the integration test GREEN; add ownership, repeated-import, clipping, geometry and policy rejection tests.
- [x] Run affected native suites, independent-reader comparison and a fresh scoped review. Record evidence and unresolved limitations.
- [x] Record the checkpoint and native API completion; retain UI work explicitly pending. No commit under the standing worktree policy.

Completed native milestone: final/verification.json under artifacts/page-content-native-20261024. Final affected suites: 272 passes, 3 ignored; 32 corrected independent cases. Review P1 appearance memory amplification corrected in one TDD pass, with byte-budget RED/GREEN, shared-context RED/GREEN and a 1,024-alias stress regression. No IPC, frontend workflow, packaged build or Windows acceptance is claimed.
