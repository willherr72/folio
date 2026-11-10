# Structured source assembly implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Preserve document structure and source identities through graph-native combination, working toward the exact combined editing/save/reopen workflow.

**Architecture:** Read-only bounded ownership inspector, source page-tree assembly, structure reconciliation, then an admitted Save route. Use existing relocation and controlled loader/writer; no renderer import repair.

**Tech Stack:** Rust, existing lopdf and PDFium only for independent rendering/acceptance.

**Spec:** `docs/superpowers/specs/2026-10-30-structured-source-assembly-design.md`, parent structured-composition design and builder dispositions.

## Global constraints

- Preserve source files, old candidates, dirty work and failed evidence. No commits/push/releases/user-app restart.
- One offline locked Cargo workload, existing shared target, one build job.
- Preserve authoritative object identities. No flattening, guard removal or fixture substitution to manufacture a pass.
- Limits and cancellation must remain explicit; no exact RSS, complete accessibility or commercial-parity claim.
- Unsupported semantics remain guarded while later adapters are built; the full combined editing objective remains unchanged.

## Review focus

- Alias references, null reverse slots, empty structures and historical unreachable owners.
- Reused tagged form content and OBJRs across pages versus conflicting parent ownership.
- Semantic cycles, malformed number-tree ranges/keys and cross-source page references.
- Attribute/alternate-text preservation and unrelated tagged constructs that do not authorize edits.
- Source language/maps/IDs and namespace collisions during merge; typed annotation/widget/form identity.

## Task 1: Bidirectional structure ownership

Files: new `src-tauri/src/composition_structure.rs` and `composition_structure_tests.rs`, internal module registration.

Interface: `inspect(document, catalog_id, page_ids, check) -> EngineResult<StructureInfo>`. Owned result records root/top children/elements, parent key/owner identity and normalized MCID/whole-object reverse entries. No mutation or modification authority.

- [x] Write red fixtures for page MCIDs with null slots, tagged link OBJR, reusable-form MCR/StmOwn, two-page shared object ownership, and arbitrary retained attributes. Add malformed/stale reverse mappings, key collisions, missing pages, cycles, cancellation and unreachable-history controls.
- [x] Implement bounded catalog reachability, canonical reference helpers, hierarchy traversal and number-tree flattening. Compare forward and reverse ownership exactly, retaining null slots and actual owner identities. Run focused `composition_structure` library tests; expected all pass after observed behavioral reds.
- [x] Add an ignored authored-corpus inspector probe for the exact public Word/Pictures inputs (explicit environment-owned path), run it locally, record hashes/counts; do not depend on private quiz files or make ordinary tests depend on artifact paths.

## Task 2: Full-source page/catalog and structure assembly

**Status:** internal assembly complete; source-bound tests and independent six/eight-page plus collision-fixture evidence are recorded in `artifacts/structured-source-assembly-20261030/multiple-checkpoint-evidence.json`. Save integration remains Task 3.

Files: graph API extensions and new `composition_catalog.rs` / tests. Consume Task 1's ownership result and exact relocation maps. Add independently testable fixtures for two tagged sources with colliding keys/maps/IDs, partial structure, language inheritance, native comments/links/shared resources and physical pages. Implement source-order page trees and reconciled catalog/structure; validate before and after. Saved output must retain identity and content under independent parsing/rendering. Do not enable unsupported features through fallback.

## Task 3: Save integration and acceptance

**Status:** in progress. Graph-native Save and the exact combined paragraph/image/page Save/reopen/re-edit session are verified by `artifacts/structured-source-save-20261030/checkpoint-evidence.json` (776 scoped native passes, fresh debug build, native desktop and independent readers). The source-language finding for unmarked content remains open; typed/occurrence adapters and broader acceptance are not complete. Resume that finding rather than redo the completed image-container correction.

Files: engine route, new integration tests and owned acceptance helpers. Preserve the byte-exact one-source path; choose the new route only after its requirements are met. Integrate complete-source blocks, crop/rotation/bookmark changes and the required graph-native adapters. Resume exact four-page baseline, then the existing combined paragraph/image/page editing workflow once lineage/occurrence adapters are verified. Record remaining stages honestly rather than claiming an internal layer completes this task.

One final implementation review after the admitted route is implemented; targeted native gates during development, broader native/build/desktop gates before runnable integration. This plan can span goal continuations; ledger completed tasks and active work rather than restarting them.
