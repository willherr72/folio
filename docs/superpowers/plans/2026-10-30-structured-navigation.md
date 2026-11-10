# Structured Navigation Implementation Plan

> Use superpowers:executing-plans under standing autonomous authority. Existing isolated worktree and dirty work are preserved. No commits, releases, per-task reviewers or deletion of evidence; one final implementation review.

**Goal:** Enable faithful original bookmarks and source-bound local navigation in graph-native combined tagged-document Save, followed by save/reopen/re-edit acceptance.
**Architecture:** Read-only canonical graph inspectors feed staged outline and destination mergers. Workspace intent chooses original preservation or a verified compatible explicit replacement. No native page-import round trip.
**Tech stack:** Rust/lopdf/PDFium native worker, existing TypeScript workspace/bookmark model, independent pypdf/MuPDF audits and Windows desktop harness.
**Spec:** docs/superpowers/specs/2026-10-30-structured-navigation-design.md, including builder rulings.

## Constraints

4000 outline items across the output, hierarchy depth32,100000 visited graph entries,1,000,000 checkpoints,16MiB staged navigation metadata. Titles at most8194 encoded bytes; portable decoded title limit4096 UTF8 bytes. Existing source parser/retained graph/output limits remain cumulative. Canonical IDs and per-source page/structure ownership only; no geometric or text heuristics. Check cancellation before staged commits and loops. Preserve original inputs and all prior evidence.

## Review focus

- Shared aliases at local versus remote/opaque destination sites: split typed local edges only, preserve other consumers.
- Name and String destination namespaces with identical bytes: resolve independently and preserve exact fit/null coordinates.
- Readable projection versus raw metadata: no lost SE/custom fields when another source's bookmark changes.
- Current generated source versus protected historical origin: inherit current outlines while enforcing original permissions.
- Mixed source languages and unsupported catalog/action features: no accidental admission or erased title semantics.

## Task1: canonical outline ownership and staged joining

Files: new src-tauri/src/composition_outlines.rs, composition_outline_tests.rs; private lib registration.
Interface: inspect(document,catalog,structure:&StructureInfo,check)->OutlineInfo{root,top,items,visible_count}; merge(document,&[OutlineInfo],check)->Option<ObjectId>. Inspect does not mutate. Merge consumes only verified infos from the private graph and stages all field replacements before committing.

- [x] Write failing tests for nested open/closed/empty trees, raw UTF16 titles, styles, canonical aliases, SE source ownership, shared actions/metadata, and source-order joining.
- [x] Add malformed parent/sibling/First/Last/count/cycle, excessive depth/count, ID overflow and cancellation atomicity regressions; run the focused tests and retain RED evidence.
- [x] Implement bounded read-only inspection, preserving all item values. Refuse unmergeable root metadata; verify counts against visible descendants. Destination/action semantics remain explicitly unapproved until Task2.
- [x] Implement one staged join with new root and only top-level parent/cross-source sibling changes. Reject overlapping input trees. Verify every changed dictionary and limits before commit; inspect the resulting outline graph again in tests.
- [x] Run all composition-focused library tests and record results. Do not enable Outlines in Save yet.

## Task2: source-bound destination namespace and action sites

Progress: composition_destinations.rs and composition_destination_tests.rs implement
read-only source namespace preparation, collision qualification, typed local operand
replacement, and bounded catalog staging. composition_navigation.rs now stages
typed action/annotation changes and commits only after validation and cancellation
checks. Both components and portable import are verified; Save admission remains Task3.

Files: new composition_navigation.rs/tests; bookmarks.rs importer and tests/bookmarks.rs; relevant composition language typed-identifier contexts.
Interface: composition_destinations::prepare borrows source namespaces; composition_navigation::reconcile stages updates, applies only after all source namespaces/typed sites validate, and returns new Dests/Names values. Compatible-reconstruction admission belongs to Task3's explicit Save intent and must inspect original outline metadata before rewriting it.

- [x] RED tests for colliding names across sources and same bytes in Name/String namespaces, shared local/remote operands, exact destination parameters, source aliases and missing/outside-page targets.
- [x] Build bounded legacy dictionary/modern tree maps with ordering/Limits checks. Qualify collisions against reserved keys; retain original destination-value objects and unknown attributes.
- [x] Inspect typed outline Dest/A, Link annotation Dest/A/AA and page AA action seeds. Traverse supported GoTo/URI/GoToR/Named and Next edges; refuse unsupported opaque/script dependencies when renaming cannot be proven. Catalog actions remain guarded. Never rewrite a generic resource dictionary by key spelling alone.
- [x] Stage typed-edge copy-on-write for shared qualified operands; preserve remote/opaque bytes. Test cancellation/size/cycle failure without mutation.
- [x] Separate legacy/modern maps in portable bookmark import and verify native/portable UI-model projection agrees with saved navigation. No second unbounded parse in composition. Actual desktop navigation remains Task3 acceptance.

Additional source-backed admission: annotation AA triggers apply beyond Link (PDF1.7 Table8.44); non-widget focus triggers are not treated as ordinary annotation triggers. Widget/action extensions that cannot be bound during qualification remain guarded. Presentation PresSteps contains a separate NA/PA navigation graph (Table8.14); it is retained exactly when names remain stable and refused on qualification until its own adapter exists. This is a remaining broad-goal capability, not completed presentation support.

## Task3: Save intent and final acceptance

Files: composition_graph/catalog, structured_source_save, document_policy/complete_source_save as needed, tests/structured_source_save, native bookmark/link tests, authored desktop/audit artifacts.

- [x] Reproduce current outlined-source Save refusal, and current-source/protected-origin bookmark inheritance issue with explicit complete-source fixtures.
- [x] Carry preserve versus compatible replace/discard intent into assembly before outline validation. Empty explicit replacement may discard unreadable outlines while preserving page navigation. Nonempty replacement with native-only metadata is refused until an origin-bound model exists.
- [x] Allow only the tested catalog features and bind language/string admission to exact typed fields; unchanged mixed-language titles must not slip through page-language scopes.
- [x] Verify exact raw graph/SE/strings, requested title edits or deliberate clear, correct local/remote link targets, source bytes, existing-output refusal atomicity, cancellation and protection.
- [x] Run scoped native suites, fresh debug build, authored combined navigation plus paragraph/image/page Save/reopen/re-edit and independent graph/text/raster/navigation audits.
- [x] Request one final implementation review; correct Important/Critical findings with RED/GREEN and affected gates. Persist source-bound checkpoint and remaining broad-goal work.

Integration ordering notes: infer unchanged intent from current complete-source blocks while retaining protected-origin authorization. Keep original per-source outline topology until language checks finish; joining top-level siblings earlier would make one source's residual-string walk traverse another source's titles. Explicit discard must exclude old outlines before that language walk. Destination identifiers require exact typed contexts; Title and unknown string metadata keep their original language guard. Inspect compatibility before reconstruction. The current inheritance helper clones/remaps source bookmark lists before aggregate validation; the new complete-block path must check cumulative metadata/count limits before cloning rather than reproducing that allocation hazard.


Final milestone: current-source inheritance, typed Save integration and two final-review corrections are verified. Final affected native gate170 passed/6ignored and fresh GUI/CLI builds passed. Windows baseline and combined editing/reopen/re-edit workflows, independent graph/raster/navigation/language checks and four negative controls passed. See artifacts/structured-navigation-20261030/checkpoint-evidence.json and review-final/coordinator-disposition.md. The earlier918-pass gate remains a separate pre-correction scope.

Acceptance clarification: desktop keyboard bookmark navigation is verified. Page-Link targets are independently resolved from saved objects; Folio canvas hyperlink activation is not implemented and remains on the broad goal. Partial/repeated/reordered tagged pages, combined forms/overlays, mixed-default titles and arbitrary native-only bookmark edits remain explicit adapters, not completed capabilities.
