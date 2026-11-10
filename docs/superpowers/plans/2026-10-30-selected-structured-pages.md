# Selected structured pages implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Preserve existing dirty work; no commits or cleanup of prior evidence.

**Goal:** Save unique selected/reordered tagged pages without losing their PDF graph relationships.

**Architecture:** Assemble contributing whole current sources once, then project the scratch graph to requested canonical page identities. Independent bounded page/structure projection precedes final graph validation and atomic publication.

**Tech Stack:** Rust, lopdf, PDFium, existing PowerShell/Python verification.

**Spec:** `docs/superpowers/specs/2026-10-30-selected-structured-pages-design.md`

## Global constraints

100,000 objects; 1,000,000 traversal steps per bounded pass; depth 64; 16 MiB staged metadata. No stream-payload clones, renderer replacement, budget increase or permission bypass. Preserve logical reading order, guard dangling navigation/opaque references and partial alternate descriptions. No repeated-page ownership or forms/overlay claims.

## Review focus

- Null/indirect inherited fields: source geometry/resources must survive reparenting (task 1).
- Cross-page elements with removed inherited Pg: retained descendants must keep original effective page (task 1).
- Shared MCR/OBJR owners: exact reverse slots, not just element membership (task 1).
- Unknown metadata or outline SE reaching dropped nodes: reject before publication (tasks 1/2).
- Current edited source versus historical security provenance: compare current bookmark targets and preserve authorization (task 2).

## Task 1: Owned graph projection

Files: create `src-tauri/src/composition_projection.rs`, `composition_projection_tests.rs`; register in `src-tauri/src/lib.rs`.

Interface: `project(Document, &[ObjectId], &mut impl FnMut()->EngineResult<()>) -> EngineResult<Document>` consumes scratch graph and preserves requested canonical page IDs/order. Private staged fields and bounded copy helpers; existing `composition_pages::inspect` and `composition_structure::inspect` prove before/after ownership.

- [x] Add tests for complete permutation and page subset; run against a refusal stub and observe expected failures.
- [x] Implement inherited page materialization, filtered structure K and reverse ParentTree projection, bounded trailer reachability and unreachable-object removal. Preserve unchanged K values and all original stream bytes.
- [x] Cover five review focus cases, sparse slots, direct/indirect K, empty elements, cancellation, duplicate selection and metadata budget refusal; run focused library gate.

## Task 2: Native Save integration

Files: `src-tauri/src/structured_source_save.rs`, `src-tauri/tests/structured_source_save.rs`, new `src-tauri/tests/support/structured_projection_save.rs`.

- [x] Add public-engine tests for selected tagged page and arbitrary interleaving/reorder; observe existing eligibility refusal.
- [x] Register unique source IDs once, reject repeated page identities, normalize complete current bookmark identities for comparison, map desired relocated pages and project after compatible bookmark handling where needed. Preserve permission, geometry, cancellation and final retained-budget checks.
- [x] Update obsolete blanket partial/interleaved refusal tests to explicit dangling-reference refusals and repeated/overlay guards. Test original bytes/output preservation and save/reopen rendering/text/tag identities.

## Task 3: Acceptance and review

- [x] Run affected composition, structured Save, bookmark, link, protection and cancellation suites sequentially; capture exact fingerprint/test inputs and terminal results.
- [x] Save selected and reordered authored Word/Pictures inputs, compare independent source-to-output text/raster/structure ownership/navigation and inherited geometry.
- [x] Build fresh custom-protocol GUI/CLI candidate and exercise organizer selection/reorder Save/reopen in owned desktop process; bind exact binary/helpers and clean only that process.
- [x] One fresh final reviewer; fix concrete findings locally with regressions, retain review and disposition. Record pending broader limits honestly in roadmap/checkpoint/continuation.
