# Source-bound List selection implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline. Standing autonomy replaces routine approval questions. One final fresh source review after actual acceptance; no per-task agents.

**Goal:** Fill genuine painted/scrolled List appearances while preserving source frames and refusing unmatched artwork.

**Architecture:** Optional per-widget native source metadata capture; bounded source/font/native-ink proof; temporary canonical answer slot feeding unchanged faithful transplantation. Existing supported paths stay unchanged.

**Tech stack:** Rust/PDFium/lopdf, current CLI/Tauri, Python pypdf/MuPDF auditors.

**Spec:** docs/superpowers/specs/2026-11-11-list-source-selection-design.md

## Global constraints

Preserve accepted041/completion73455032bd8d8e328d2d1ab266bf665afbafbb565b667e0ac4740489039ae74e and its launcher. Baseline producte7a064507ce056a275af99ed019541c97ea82a634c057e4c6d19cf56f7f49132/version041. No public schema/dependency/version/history/cache changes, commit/push/merge/publication or user-app auto-launch. Existing4MiB/262144-operation/64MiBcopy guards unchanged. Optional proof capture64MiB; one isolated32MiB font/appearance probe with250000native-character bound, no full-source parse/clone. Native models come from before filling. No guessed selection/row/font identity, palette heuristic, clip union or output-derived mask. Existing native inspection remains unchanged; no choice-editor4096-byte/128KiB cap added to ordinary fill. OneCargojob/D:cache/temp; serialized suites/builds/UI. Qualified frontend reuse only on exact input equality. Fresh full native correctness under explicit profiles after final changes. One final fresh source review after actual native/desktop acceptance.

## Review focus

- Linked widgets/repeated placements with differing original selection,TIor inherited font context must never borrow the first widget's proof.
- Duplicate/empty labels, V/I disagreement, inconsistentTopIndex and hidden/nonpainting glyphs must not authorize removal of arbitrary source artwork.
- Graphics restore/path state, interleaved paints, shifted clip origins and transformed source text must preserve layer/state and source-fixed boundaries.
- Budget exhaustion, huge strings/fonts/glyph counts and source/model mismatches must refuse atomically without degrading already-supported paths or leaking native probes.
- Actual rendered row paint and raw native-reference state must agree; auditors must reject self-authorizing output masks/TI, empty-paint false positives and stale or wrong candidates.

### Task 1: source evidence and private List proof

**Files:** Create src-tauri/src/form_list_source.rs,form_list_source_model.rs,form_list_source_ink.rs and form_list_source_tests.rs; engine_list_source_tests.rs. Modify form_answer_appearance.rs(private module/export),form_fill_identity.rs(borrowed selected-widget placements),engine.rs(test module). Test form_compound_answer.rs(new requested positive cases, custom-art negative). Artifacts in artifacts/list-selection-proof-20261111; baseline copies/manifests and evidence. Do not connect publisher yet.

**Interfaces:** CapturedListSources::capture(native:&NativeForm,source:&Document,widgets:impl iterator of(ObjectId,&[(usize,usize)]),enabled:bool)->CapturedListSources captures optional, budgeted native-before state for legacy-ineligible slots; private fields bind source address/widget/AP identity. prepare_list_source(pdfium:&Pdfium,source:&Document,original:&Stream,resources:&Dictionary,widget:ObjectId,captured:&CapturedListSources,budget:&mut FillAppearanceBudget)->EngineResult<Option<Stream>> first preserves the old supported path, otherwise returns a fully proved canonical temporary source stream. Missing proof never authorizes an edit. Internal model/row/probe types stay private. Add selected_widget_placements(&self)->impl Iterator<Item=(ObjectId,&[(usize,usize)])> without cloned placement trees.

- [x] Preserve accepted041authority, baseline source/native/frontend maps and touched bytes; retain the fresh diagnostic4success/4refusal results. Expected: no active compiler/UI, original041launcher and source maps match.
- [x] Before product implementation, split the old four-case refusal test into the ambiguous-artwork negative plus three genuine painted/scrolled positive fill/clear/refill/save/reopen cases, using current Engine APIs. Run --test form_compound_answer list_source with the normal opt0correctness driver. Expected: exactly the three new cases fail at the current compound-artwork guard; custom-art negative passes. Preserve all logs/source/input receipts; this is the public behavioralRED.
- [x] Write private capture/proof tests before implementation: native-before state bound per widget; source/doc/widget/AP mismatch; exhausted optional budget preserving valid old slots; exact source labels/TI/order; duplicate labels byindex; hidden/missing glyphs; custom/partial rectangles; single clip/restore boundary; actual native glyph ink; capacity/byte/operation/glyph/probe limits. Add source-model helper interfaces then implement optional capture, row proof and one bounded native probe. Expected: missing private APIs fail only for the new interface before implementation; meaningful publicRED above remains the trigger authority. Private --lib list_source becomes green while publisher integration is deliberately not connected.
- [x] Test canonical state replay removes selected rectangle paint rather than duplicating it, preserves following colors/frame order and keeps old supported sources byte-path compatible. Exercise direct/inheritedTI, splitTJzero adjustments, separateBT/ET, shifted origins, ordinary axis-preserving transforms and standard/verified embedded font bindings. Expected: all private list_source tests pass; no public feature-success claim yet.

### Task 2: faithful fill integration and current-source native gate

**Files:** Modify src-tauri/src/form_fill_publish.rs only for optional pre-fill capture and per-widget prepared source; complete form_compound_answer.rs and engine_list_source_tests.rs. Artifact drivers/manifests/native-reference outputs/full-native receipts.

**Interfaces:** Task1capture runs before native_form.fill for retained source List APs, skips globally regenerated/no-AP paths, and does not affect no-op. After existing contexts.bind, Task1prepare receives that exact widget/resources; unchanged transplant publishes prepared-or-original source. Existing generation, actions, permissions, native state witness and graph installation remain mandatory.

- [x] Confirm the preserved publicRED's tested publisher remains unchanged; if a changed publisher makes that evidence inapplicable, rerun the three failing cases before wiring. Connect capture/preparation only for List. Expected: all three positive regressions and ambiguous-artwork negative pass with exact source bytes, static metadata/frame invariance and bounded repeated graph size.
- [x] Run affected List/Combo/Text/context/import tests. Add linked-widget/repeated-placement mismatch, source-empty ambiguity, legacy-supported-budget exhaustion, wrong font/native glyph, offscreen selection and malformedTI regressions. Capture raw native-only reference PDFs/TIfor independent expected-state manifests under a test-only output directory; never add a production bypass. Expected: source/model ownership and original safety tests stay green; typed refusals leave no published document/output.
- [x] Capture final source/native input maps and run full current library once at Folioopt0 plus all optimized integration/bin targets and doc tests, with exact before/after guards and parsed complete totals. Expected: zero failures/compiler errors/unfinished targets, explicit original profiles/ignores; no reused old-source native pass or timeout relaxation. Failures require diagnosis and changed-source checks before acceptance.
- [x] Verify exact equality of current frontend/shared inputs to accepted0411517/226evidence; otherwise run fresh frontend. Build fresh normal optimized GUI/CLI/full25resources from final source, preserved attempts and exact fingerprints. Expected: successful normal builds, exact renderer/resources and no concurrent source edits; no package/launcher promotion.

### Task 3: actual CLI/desktop acceptance, review and historical checkpoint

**Files:** New source-bound expected manifests, CLI/desktop drivers and independent auditors/tests under artifacts/list-selection-proof-20261111; public frozen review, dispositions, historical component authority and completion record. Update repository scope notes after sealing.

**Interfaces:** Task2source-bound binary/native-reference outputs feed actual current CLI and trusted-input desktop runs. Auditors consume frozen source/request/native-reference expectations, never derive authorization from output paint/geometry/TI.

- [x] Write independent audit corruption controls first: blank paint despite logical answers, wrong selected rows, missing label, wrongTI, changed border/outside pixels, expanded self-authorizing mask, wrong font/glyph identity and incorrect option/default/flag topology. Expected: corruption is rejected before green audits; positive controls read actual saved appearance/ink at72/144dpi.
- [x] Run CLI fill/clear/refill/save/reopen/re-edit on the three painted/scrolled sources, linked/differing contexts and boundary fixtures. Keep original ambiguous-artwork and new unmatched/invisible/invalid cases refused. Expected: exact wanted values/indices/native-reference scroll state, actual labels/selected paint, source-fixed outside RGB and no obsolete graph growth/source mutation.
- [x] Run compiled trusted-input desktop draft Reset/Discard cancellation/Apply/Undo/Redo/save/reopen on single,multi,scrolled and linked cases; CLI separately exercises actual candidate Preview and MCP exercises Discard; independent saved-file audits and rawFILETIME owner/profile cleanup. Expected: all actual source-bound output identities, font/resource proofs and fixed masks pass without direct native edit UI bypass.
- [x] One final fresh source review after concrete evidence. Resolve consequential findings inline with RED/GREEN/affected verification; record every minor/declined/ruling/cost, no routine re-review. Archive product/input/helper/reference/output bytes and verify historical corruption/independence, then seal component. Expected: concrete scoped completion, accepted041launcher unchanged, originals/failures preserved, no broad parity or portable release claim.
