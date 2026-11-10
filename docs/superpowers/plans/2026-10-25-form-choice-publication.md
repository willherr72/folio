# Faithful choice publication implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline. The user's autonomous-work instruction supersedes routine approval pauses. One whole-feature review follows public integration; do not dispatch per-layer reviews.

**Goal:** Turn the existing choice model and native generator into an atomic complete-source edit that preserves every linked widget and unrelated PDF object.

**Architecture:** Validate the immutable source before staging changes. Generate and verify original appearances, then generate changed text in each widget's actual font, map it into the original appearance's coordinate system, and replace only its isolated text block. Publish bytes only after all linked widgets and the resulting native field state pass verification.

**Tech stack:** Rust, lopdf, pinned PDFium v4, existing bounded graph copying and text-block isolation.

**Spec:** `docs/superpowers/specs/2026-10-25-form-choice-design.md`.

## Global constraints

- Preserve user PDFs, prior binaries, dirty worktree and failed evidence; no commits or publication.
- Baseline native regression80037 is COMPLETE:1,114passed/0failed/11ignored across100targets; both exit codes0. Baseline fingerprint:c7236c34ab83d076885a3decfefe59b7657bac5693105e9bfe57e5585de724cd. Do not restart that completed run. Current geometry code subsequently passed the full334-test library suite (3ignored); see geometry-verification.json for its distinct fingerprint.
- 2,000 options, 4,096 UTF-8 bytes/string and128KiB aggregate model text; source inspection's existing raw/graph limits still apply.
- Private generator32MiB, original-font graph copy/comparison16MiB, appearance256KiB/16,384operations. No font substitution or source-global flag rewrite.
- Original `BBox`, `Matrix`, resources, artwork, rollover/down appearances, actions and unrelated field settings remain intact.
- Caller-supplied PDF object IDs or resource identities never authorize publication.

## Geometry evidence and decision

`artifacts/form-choice-publication-20261025/geometry-spike/results.json` contains20 native-generation/MuPDF cases: combo/list, rotations0/90, canonical/shifted/scaled/affine-matrix appearances and stale-color negative controls, rendered at1x/2x/4x.

Direct text copying changes all shifted/scaled cases. Mapping the generated BBox into the original BBox preserves exact pixels in all16 compatible cases, including an existing affine outer Matrix. The four stale-color cases still differ and must fail unchanged-regeneration admission. This is a controlled spike, not product acceptance or a proof for arbitrary source graphics.

Preserve the original outer Matrix. Use an inner axis mapping from generated BBox `g` to original BBox `s`:
`sx=(s.right-s.left)/(g.right-g.left)`, `sy=(s.top-s.bottom)/(g.top-g.bottom)`, `tx=s.left-sx*g.left`, `ty=s.bottom-sy*g.bottom`.
Wrap only the generated Tx interior in `q sx 0 0 sy tx ty cm ... Q`, leaving its BMC/EMC and the source's before/after artwork in place. Do not cancel the original outer Matrix, which can express intended rotation/skew.

## Review focus

Additional reproduced proof requirements: `transparency-spike/results.json` has12 PDFium/MuPDF controls showing that white-page comparison hides white text removal while transparent comparison detects it. `physical-proof-spike/results.json` has3 native controls: at UserUnit128, different A/B glyphs at raw font size0.02 compare equal at raw1x/2x/4x but differ at every corresponding physical scale. These are synthetic sampling controls; retain both as regressions in the actual widget implementation.

1. A linked widget's AP or resources may alias an unrelated field; clone only the affected widget's AP dictionary/normal stream and preserve all aliases outside it.
2. Empty fields cannot provide glyph positions for geometry inference; use the explicit BBox mapping and proven empty background, never infer placement from absent ink.
3. Matching source/generator font names do not prove identity; retain current graph/code proofs for every widget, including hidden labels.
4. NeedAppearances=true requests reader-wide regeneration; refuse unsupported choice publication precisely, preserving the source flag and other fields.
5. Pixel comparison must not allocate unbounded images or inspect only a crop that hides moved text/artwork; compare the complete isolated appearance under the original annotation transform and charge aggregate work first.

## Task1 — geometry mapping and unchanged-source proof

**Files:** new `src-tauri/src/form_choice_appearance.rs`; modify `form_choice_text.rs` for bounded mapped transplantation; new `engine_choice_publication_tests.rs` wired in `engine.rs`; module registration in `lib.rs`. Expose a narrow `engine::source_unit_from_pdf(&Document,ObjectId) -> EngineResult<f32>` wrapper around existing `physical_units::page_unit`, retaining its exact page-local/inheritance/limit policy.

**Interfaces:**
- `ChoiceTextBlock::transplant_mapped(&self, generated: &Self, names: &BTreeMap<Vec<u8>,Vec<u8>>, mapping: [f32;6]) -> EngineResult<Vec<u8>>`. Existing `transplant` delegates with identity, preserving current behavior and empty-slot ordering.
- `form_choice_appearance::geometry(source: &Document, original: &Stream, generated: &Document, replacement: &Stream) -> EngineResult<[f32;6]>`. Resolve direct/indirect arrays and coordinates; finite positive BBoxes, finite invertible original/generated Matrices, bounded resulting mapping. Keep the source Matrix rather than substituting the generated Matrix.
- `form_choice_appearance::verify_unchanged(pdfium: &Pdfium, source: &Document, widget: ObjectId, page: ObjectId, candidate_content: &[u8], budget: &mut AppearanceBudget) -> EngineResult<()>`. The page is discovered from source-owned widget locations, never supplied as a client PDF object ID; resolve its UserUnit through the shared engine wrapper.
- `AppearanceBudget::default()` limits the entire transaction to64million rendered pixels and32MiB retained staged appearance bytes. Individual bitmap dimensions must fit the renderer and each bitmap must remain at most16million pixels. Count both source and replacement at1x/2x/4x before allocation; do not downsample oversized proofs into acceptance.

- [ ] Archive source baseline and copy trusted geometry fixtures into focused test support with explicit provenance; do not run tests until80037 is terminal.
- [ ] RED tests: `shifted_scaled_and_rotated_choices_keep_original_geometry`, `affine_outer_matrix_and_foreground_artwork_survive_text_changes`, `indirect_geometry_is_resolved_and_invalid_geometry_refused`. Assert exact no-op pixels, original BBox/Matrix/resources, and moved/changed text only where expected. Include singular/nonfinite/empty arrays and excessive mappings.
- [ ] Implement mapped transplantation with preallocation byte/op/depth accounting. Identity must avoid introducing unnecessary operators. Empty-slot insertion must retain the original text layer. Nested transform wrappers count against the existing depth32 limit.
- [ ] Build two bounded, action-free, one-page appearance probes using Copier and the original annotation-to-page transform. Render both using the shared Pdfium runtime at1x/2x/4x onto transparent backgrounds and require exact RGBA pixels at every scale. Compare complete widget appearance, including all artwork and clipping. Do not flatten onto white, which would hide white text or alpha differences. Return a precise source-appearance mismatch on failure.
- [ ] Normalize each probe page to the widget's raw width/height. Transform all four original BBox corners with the original Matrix, then match those transformed bounds to that page, as PDFium's `cpdf_annot.cpp::AnnotGetMatrix` does. The original Form Matrix remains in the copied stream and is applied once by `Do`. Set fixed bitmap dimensions from `ceil(raw_dimension * source_UserUnit * scale)` after finite/range/budget checks. Use a budgeted zero-initialized BGRA byte buffer with `PdfBitmap::from_bytes`, `render_into_bitmap_with_config`, and `clear_before_rendering(false)` so transparent initialization is guaranteed without relying on the wrapper's unchecked FillRect result. Apply explicit width/height scale factors equal to admitted pixel dimensions divided by the private page dimensions: fixed bitmap size alone leaves the matrix render path at scale1. Keep form rendering disabled. Drop each bitmap before accessing its borrowed byte buffer; compare identically configured complete BGRA buffers including alpha, avoiding extra RGBA copies. Never accidentally apply UserUnit twice or inherit it from a Pages node.
- [ ] RED/green tests: `stale_size_color_alignment_or_answer_cannot_be_silently_repainted`, `empty_native_fields_can_receive_text_at_the_original_layer`, `appearance_budget_is_charged_before_rendering`. Verify source bytes/objects untouched after each refusal.
- [ ] `transparent_white_text_does_not_pass_a_white_background_only_proof`: a saved white answer and an empty replacement must differ even though both look blank on white paper. Preserve alpha in both the comparison and its test oracle.
- [ ] `linked_widgets_use_their_own_physical_page_scale`: UserUnit0.5/1.7/8 and indirect page-local values retain identical original appearance; a reused widget on pages with distinct units is checked at every distinct physical scale. Missing/invalid/ancestor-only UserUnit follows the existing shared policy. Budget overflow at a large physical scale refuses rather than rendering a tiny substitute.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --lib choice_ -- --test-threads=1`; preserve logs and independent MuPDF comparisons.

## Task2 — atomic all-widget transaction

**Files:** new `form_choice_publish.rs`; modify `form_choice_draft.rs` and `form_choice_probe.rs` only for reusable ownership/inspection interfaces; extend `engine_choice_publication_tests.rs`.

**Interfaces:**
- Factor `prepare_choice_edit_context(ctx: Context, request: &SetFormChoicesRequest) -> EngineResult<PreparedChoiceEdit>` out of the existing byte wrapper. It consumes one loaded Context after original validation, avoiding a second full-document clone/decode. Existing public `prepare_choice_edit_pdf` remains a wrapper and retains its behavior.
- `form_choice_publish::change(pdfium: &Pdfium, bindings: &dyn PdfiumLibraryBindings, bytes: &[u8], request: &SetFormChoicesRequest) -> EngineResult<Vec<u8>>` returns a complete candidate only; no registry, filesystem or original-source mutation.

- [ ] RED `every_linked_widget_is_verified_before_candidate_publication`: two widgets with distinct local fonts, second widget unsupported. Assert whole operation refuses and source reopens unchanged. Include one widget reused on multiple pages and inherited/shared AP resources.
- [ ] Load/inspect Context and source key once. Validate global NeedAppearances absent/false; true or malformed is an explicit unsupported publication reason. Preserve document/field actions unchanged and never execute them; retain existing authoring preflight restrictions. Verify source native V/I interpretation agrees with the bounded choice model before modifying it.
- [ ] For each original widget, generate the original state with existing font/code proof, map its Tx into the saved appearance, and run unchanged-source proof. Require original and replacement Tx font names to resolve in the saved normal appearance's own Font resources; the generator can use an AcroForm DR fallback, but a standalone appearance probe must not accept two blank renders caused by missing font bindings. Preserve the resources and report unavailable bindings explicitly rather than injecting a replacement font graph. Keep only bounded proof witnesses; discard private generator documents after each widget.
- [ ] For a widget placed on several source pages, validate every distinct `(widget, UserUnit)` pair. Deduplicate identical source-bound pairs before charging repeated render work; do not borrow another widget's font, no-op or physical-scale witness.
- [ ] Consume that Context with `prepare_choice_edit_context`. Generate staged state for every widget, including options/defaults that do not require repaint, to retain all-label coverage. Repaint only when required. Charge retained content/AP clones across all widgets before allocation; source AP dictionaries and resources stay unchanged until all candidates are ready.
- [ ] Install each new normal stream through a new per-widget AP dictionary while preserving R/D and the original normal dictionary/resources/BBox/Matrix. Do not mutate a shared normal stream or copy the generator's graph/global flags into the source. Prune only unreachable objects using the established bounded helper, then serialize through BoundedWriter.
- [ ] Reinspect the candidate and compare complete expected choice model and every linked widget location. Check native field answers/options/top index against the staged model; no mismatch may become registered bytes. Preserve unrelated field models and native values. Pure no-op returns the exact original bytes; metadata-only edits leave saved AP operands unchanged.
- [ ] Tests: `shared_normal_appearance_does_not_modify_unrelated_fields`, `metadata_only_defaults_keep_original_appearance_bytes`, `clear_restore_and_reorder_preserve_answers_and_top_index`, `global_regeneration_policy_is_not_silently_cleared`, `native_selection_disagreement_is_refused`, `missing_local_font_bindings_cannot_authorize_blank_noops`, `late_failure_never_returns_partial_candidate`.
- [ ] Run focused native tests plus affected form/model/inspection/property/font tests and independent object/font/pixel checks, with source fingerprint before/after. No new public endpoint until this whole transaction passes.

## Task3 — complete-source worker and adapter

**Files:** new `src-tauri/src/form_choices_engine.rs`; modify `engine.rs`, `app.rs`, `lib.rs`, `src/editor/adapter.ts`; new `src-tauri/tests/form_choices.rs` and `src/editor/form-choices.ts`.

**Interfaces:**
- `PdfEngine::inspect_form_choices(&self, source_id: &str, page_index: usize, annotation_index: usize) -> EngineResult<FormChoices>`.
- `PdfEngine::set_form_choices(&self, source_id: &str, request: SetFormChoicesRequest) -> EngineResult<DocumentInfo>`; corresponding worker requests and Tauri commands `inspect_form_choices`/`set_form_choices`.
- Optional `FolioAdapter.inspectFormChoices(sourceId,pageIndex,annotationIndex): Promise<FormChoices>` and `setFormChoices(sourceId,request): Promise<DocumentInfo>` with exactly matching Rust serde types in `form-choices.ts`. The enum payload property is `value`, not the older native FormFieldValue's `text`/`indices` properties.

Inspection of `form_properties_engine.rs`, `form-filling.ts` and App confirms that logical-field edits return a whole new DocumentInfo. Do not use an occurrence-specific page replacement: changing the field's options affects every linked appearance, including repeated workspace occurrences. This corrects the earlier proposed occurrence endpoint in this plan.

- [ ] RED integration tests for fresh source registration, unchanged page count/dimensions, all linked widgets, repeated workspace occurrences, bookmark/protection/recovery metadata retention, stale source key and denied editing/annotation permissions.
- [ ] Follow the existing form-properties worker's `editing_document` and annotation-policy checks, preserved original path/bookmarks/policy, `open_bytes` registration and physical-point DocumentInfo conversion. Call the verified transaction once. No source registration occurs on error; the selected source remains usable.
- [ ] Roundtrip through the public engine: Preview candidate opens in native rendering, original remains unchanged, candidate later fills correctly, saved candidate reopens with correct choices/current/default values, and unrelated fields and actions retain their original graphs. Exercise protected-owner and denied-user cases using existing fixtures.
- [ ] Expose only the matching Tauri commands and adapter methods. Run relevant Rust integration tests and adapter serialization tests before UI adoption.

## Task4 — editable choices with native preview and one-step Apply

**Files:** `src/editor/form-choices.ts`, new `src/editor/form-choices.test.ts`, new `src/components/FormChoicesDialog.tsx` and stylesheet/test file; modify `src/App.tsx` and existing form tool entry controls/tests.

**Interfaces:** the dialog receives producing adapter, captured source/page/field, a live `isBorrowed(id)` predicate, `onApply(candidate: DocumentInfo): Promise<void>` and `onClose()`. It owns native preview candidates until successful adoption. App uses `commitFormReplacement(session,capturedDocument,sourceId,candidate)` as one Undo step and updates the active form tool to the new immutable source.

- [ ] Draft model uses stable local row identities plus original `sourceIndex`, separate display/export values, and explicit current/default Keep/Clear/Rows/Text actions. Serialization uses current row order, preserves original identities and checks4,096byte/string,2,000row,128KiB limits before native calls. Tests cover duplicate labels/exports, empty values, Unicode byte counts, removal of selected/default rows, free text and absent-V default semantics.
- [ ] Use an option list with one selected row's Display label/Export value editors and Add/Remove/Move up/Move down controls, rather than thousands of simultaneously mounted input controls. Preserve the selected row and focus on reorder. Show original current/default choices separately from the row being edited. Removing an in-use row requires an explicit replacement or Clear action; never silently change an answer.
- [ ] Native Preview calls `setFormChoices` and renders the returned complete-source candidate. Show linked-page navigation, original versus changed state and clear errors. Invalid or changed drafts invalidate the prior preview immediately. Apply adopts that exact candidate without regenerating it. Reset restores the complete original draft and clears transient invalid input state.
- [ ] Validate candidate identity against the live workspace before accepting ownership. Reject a borrowed/malformed source ID without closing another tab's source. On cancellation, superseded preview, late response, adapter replacement, unmount or failed adoption, close only an owned candidate through its producing adapter. Close each owned candidate exactly once; keep adopted IDs alive for history.
- [ ] App admission requires current captured tab/adapter/document, no dirty form answer or concurrent operation, desktop capability, and editing/annotation permissions. Keep frontend checks in addition to native enforcement. Use the existing complete-source commit path for all repeated source pages and one Undo/Redo step.
- [ ] Tests cover preview invalidation races, out-of-order responses, borrowed-source response, adapter replacement, failed/stale Apply, cleanup, Reset, linked navigation and focus. At640px width controls remain reachable; keyboard can edit pairs, reorder, choose current/default values and activate Preview/Apply/Cancel. Exercise2,000rows without truncation.

## Task5 — whole-feature acceptance

- [ ] Run one final whole-feature review with the complete source delta, prior findings/decisions and actual evidence. Fix findings inline without per-layer review rounds or another review loop.
- [ ] Complete affected native and frontend suites, build a new Windows executable, and verify open/edit/preview/Apply/Cancel, linked pages, later filling, Undo/Redo, save/reopen and crash recovery. Include640px/keyboard acceptance and negative unavailable-font/source-appearance cases.
- [ ] Independent raw-object/font/pixel checks cover normal and transformed widgets, artwork, aliasing and preserved selections/defaults. Record exact source/binary hashes and all limitations. Renderer v4 also requires broad native and desktop checks beyond this new feature; preserve the old accepted executable throughout.
- [ ] Update product checklist and execution ledger against actual completed capabilities and remaining gaps. This is a working choice-editor milestone, not Adobe/Foxit parity or an authorization to release/publish.

## Current execution checkpoint

Task1 and Task2 native publication are implemented. The affected native gate
passed437tests/0failed/3ignored across10targets, with a matching source fingerprint:
26ec38fb62d3c9b8590bb6f933fd86c7ca02876b7721b05baf2fc5bdb6ebaf31.
Evidence: artifacts/form-choice-publication-20261025/transaction-regression/verification.json.
All11transaction tests pass, including16transformed cases, repeated-edit stability,
source aliases, metadata-only edits, empty/free answers and reused widgets at
distinct physical scales. Independent pypdf/MuPDF checks cover22candidate pairs,
44pages at1x/2x/4x, geometry/resources/artwork, restoration and raw field values.
The32MiB aggregate retained-appearance budget is implemented and exercised.

Task3 worker/adapter integration is implemented. Four service tests reached the
expected missing-publication failure before implementation. Corrected integration
tests4/4, adapter tests5/5 and TypeScript checking pass. The initial recovery test
omitted required workspace metadata; corrected to use the application's complete
workspace shape. Save/reopen, later fill, original path recovery, repeated source
occurrences, bookmarks/actions, physical dimensions and protected-owner copies
are covered. All runs are terminal. Evidence is in worker-checkpoint/verification.json
under the same artifact directory. Task4 dialog, app history integration, final
review and desktop acceptance remain; no working choice UI or new EXE yet.

Context.widgets comes from a HashMap and is unordered. Resolve page annotation
slots from FormChoices.widgets; never pair ctx.widgets[0] with page0 by assumption.
The native generator uses WorkerRuntime.bindings, not a Pdfium.bindings() method.
The437-test fingerprint precedes the new public worker and adapter integration;
it must not be used as evidence for those later changes.


### Corrected choice editor desktop checkpoint (2026-10-26)

Task4 complete; choice-specific Task5 checks complete. One final review returned
two P2 findings. Both reproduced and fixed inline, without re-review: empty Rows
decisions now require a row or Clear; duplicate-export default values disclose
matching rows and reset ambiguity. RED4 -> GREEN25 for empty actions; RED2 ->
GREEN35 across four focused files for ambiguity. Full frontend1260/190files passed.
Corrected Windows build and source-fingerprint check0; fingerprint:
0410f39546b0604cc9015b1ca50d16a7d359588f54bc64cbce07199bd16e6c42.
EXE: 3ae33763abc4490b9a7464be710f42a905bab22a411928c8035e3de0aaf8fa0f.
Renderer: ae6b07c4a7dc0f17be3937acf73761e59924e703777eb99657fb07a1149ead6a.
Prior accepted EXE preserved. corrected-build is a tested development checkpoint,
not a release or broad parity approval.

Windows desktop-run-4 passes real dropdown/list/free-answer editing, linked native
preview, Cancel, repeated-page Undo/Redo, 640px controls, keyboard reorder focus,
save/reopen/later fill, explicit row decisions, default ambiguity disclosure and
unsupported-font draft retention. Exact candidate identity remains covered by
unit/App tests; the desktop harness does not claim its failed IPC observer worked.
Crash restore passes all7tabs, unsaved Recovered Alpha, Later answer and repeated
Combo edits. Independent pypdf/MuPDF checks cover7saved pages at1/2/4x with exact
outside-widget pixels, values/defaults and source preservation. Recovered values
and original embedded Latin/Greek font programs independently verified.

Harness failures retained: run1 exact label lookup included option text (use
accessible role/name); run2 nonwritable Tauri invoke prevented observation;
run3 older Save helper missed hidden Shell filename control (existing
set-release-dialog helper passes). run4 initial startup race is separately logged;
subsequent run used confirmed live process and passed. Independent audit initially
compared pypdf NullObject with Python None; corrected explicit-PDF-null handling.
No product guards weakened. Greek probe with NeedAppearances=true correctly
refused; a separate synthetic fixture removed that policy while preserving its
already-generated appearances, and passed. Original fixtures were preserved.

Task5 broader renderer acceptance remains LIMITED: supplied quiz successfully
inserts 'this', previews/Applies, Undo/Redo, save/reopen and faithful re-edit. But
independent MuPDF inspection finds first-line baseline moves upward3pt; unsorted
extraction appends the paragraph and sorted extraction moves nearby 'a.' after it.
Outside-paragraph pixels exact at2x; word counts differ by precisely one 'this'.
Record this as the next fidelity investigation, not a passed exact-placement or
reading-order check. Comparison against the prior accepted binary is not yet run;
attribution to renderer v4 is unproven.

Ruling: retain positive choice acceptance separately from broader paragraph
acceptance -- the new observation is outside the choice delta but gates a fully
accepted general-purpose build -- cost: another focused compatibility cycle before
promotion. Review/dispositions: artifacts/form-choice-final-review-20261026.
Runtime evidence: artifacts/form-choice-ui-20261026. No commits/push/publish.
Goal active. Next: reproduce and trace quiz baseline/extraction-order drift against
prior accepted binary, then fix with a focused before/after regression.

Later comparison: quiz-baseline-comparison.json proves the corrected choice EXE
and prior accepted EXE produce identical 2x pages and first-line baselines. The
3pt shift therefore predates renderer v4. Likely cause: fresh area top uses
font size while styled layout subtracts measured ascent (12pt versus9pt here).
See artifacts/form-choice-ui-20261026/paragraph-followup.md. No follow-up product
changes yet; next goal cycle starts from that reproduction, not choice rework.
