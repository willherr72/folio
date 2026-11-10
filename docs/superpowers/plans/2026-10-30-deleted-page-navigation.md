# Deleted-page navigation implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for inline execution. Do not commit, alter prior packages or discard inherited work.

**Goal:** Delete supported PDF pages without losing surviving native bookmarks or publishing broken local navigation.

**Architecture:** Extend the existing source-bound destination/action walker with selection semantics and original-object outline pruning, then integrate with the owned page/structure projection and final reachability checks.

**Tech Stack:** Rust/lopdf/PDFium, existing native test engine and PowerShell/Python/Playwright acceptance tools.

**Spec:** `docs/superpowers/specs/2026-10-30-deleted-page-navigation-design.md`

## Global constraints

Preserve Name/String namespace distinction, raw surviving metadata, source permissions and atomicity. Limits:100,000 objects;1,000,000 traversal steps/values;16MiB staged navigation;action/outline depth32;structure/page depth64;4,000 outline items. No stream cloning, budget increases, renderer substitution or silent destination redirection. Removed actions include their dependent Next branch; retained siblings preserve order.

## Review focus

- Shared aliases consumed by multiple action contexts must not receive inconsistent rewrites (task1).
- A cached action at a deeper caller must not bypass depth limits (task1).
- Closed outline parents with surviving children must keep signed Count semantics (task2).
- Surviving opaque metadata referencing a dropped outline node must refuse, including aliases (task2).
- Native bookmark selection intent must not authorize unrelated portable edits or stale-source changes (task3).

## Task 1: Typed destination and action selection

Files: `composition_destinations.rs`, its tests; `composition_navigation.rs`, its tests.

- [x] Add red tests for canonical destination target lookup and selected catalog pruning with separate Name/String maps and remote operands.
- [x] Expose bounded `Prepared` target/filter APIs; preserve original surviving entry values and namespace identity.
- [x] Add action selection mode and explicit removed result, retaining atomic staged changes and copy-on-write cache. Test primary deletion, Next branch deletion, sibling ordering/sharing, unsupported contexts, cycles/depth/cancellation/budgets.

## Task 2: Native outline projection and graph integration

Files: create focused `composition_outline_projection.rs` + tests if separation helps; extend `composition_navigation.rs` and `composition_projection.rs`; register module.

- [x] Add red tests for native-metadata outline deletion/folder retention, SE loss and surviving external references to removed nodes.
- [x] Stage original hierarchy pruning and signed count/sibling changes; preserve raw titles/private fields and original kept identities. Return removed IDs.
- [x] Invoke typed navigation projection with original page/structure inspection and removed elements before final page/tag reachability checks; revalidate selected output navigation/outline.

## Task 3: Native Save and accepted producer workflows

Files: `structured_source_save.rs`, `tests/support/structured_projection_save.rs`, new focused native navigation-selection test support if needed; artifact scripts under `artifacts/deleted-page-navigation-20261030`.

- [x] Reproduce public Save refusal with incoming link and native-only bookmarks; select preservation intent only when the requested portable list matches the current bounded selected projection. Explicit discard and unrelated edits keep prior semantics.
- [x] Run affected native composition/bookmark/Save/protection/link tests, including stale/unsupported identity and destination-preserving refusals.
- [x] Save selected authored navigation inputs; independent target/namespace/raw outline/action-sharing/tag/pixel/text audits with mutant controls.
- [x] Build source-bound GUI/CLI candidate; owned desktop organizer deletion, Save/reopen and surviving bookmark activation; independently audit desktop files and close only the owned process.
- [x] One final review, local correction pass if needed, checkpoint/roadmap/continuation with remaining limits. Keep broad goal active.
