# Mixed paragraph publication implementation plan

> Execute inline with the existing execution/TDD workflow. No routine approval
> gate; user authorized continuing the full PDF-editor quality goal.

**Goal:** Publish mixed source-font paragraph replacements into immutable derived
PDF pages, then verify actual reopened output and save/export fidelity.
**Spec:** `2026-10-23-mixed-paragraphs.md`, milestone 4. Explicit style overrides,
versioned memory and GUI remain subsequent dependent work, not removed scope.

## Design

Reuse verified inspection and the layout kernel through a shared private planner
returning Prepared, original raster, and placements. Preserve each bound font's
actual resource key. Replace selected source shows with the existing cursor-only
advances so relative unselected text stays positioned. Append isolated text
fragments with explicit font/size/color/text-state/matrix settings.

Reopen before publishing. Verify remaining text style/bounds/glyph identities and
positions, expected fragment count, encoded replacement extraction, source font
program/proof, color, effective size, matrix/origin, and each visible character's
origin from verified advances. All visible glyph ink must fit the requested area.
Retain neighboring-text/graphics collision refusals, solid background allowance,
source-copy raster check, outside-change-area pixels, and native comments. Preserve
source policy/credentials and original-path protection in the new document handle.

No mutation of the input or publication on a failed proof. No mixed re-edit metadata
or GUI command until those dependent stages are verified. A separate diagnostic
example may exercise the new native preview/export path for independent readers.

## Steps

1. [x] Add failing `src-tauri/tests/styled_paragraph_publication.rs` integration
   tests for actual styled replacement, source/neighbor preservation, native
   notes, all rotations, export/reopen and refusal cases.
2. [x] Retain resource_name on BoundStyle in `text_paragraph.rs`; extract private
   prepare_styled_paragraph in `styled_paragraph_plan.rs` so read-only layout and
   publication share validation/metrics. Add PreviewStyledParagraph worker/API.
3. [x] Implement isolated source-font emission and reopened output validation in
   `styled_paragraph_publish.rs`. Keep uniform publisher unchanged. Preserve source
   policy, comments and source bytes; do not add style substitution in this step.
4. [x] Cover embedded subsets, whitespace-only fragments, hard/soft breaks,
   justification, missing glyphs, protected sources, graphics and text collisions.
   Exercise a second native edit where admitted without claiming faithful memory.
5. [x] Independently inspect and render saved Folio outputs. Run appropriate native
   suites, scoped review, reconcile defects, record source/output fingerprints and
   exact limitations. No GUI executable is packaged for an unconnected API.

Review focus: per-style resource aliases/identity, transformed source effective
sizes, output object counting including whitespace, precise glyph advances across
fragments and justification, comment restoration, rights propagation, and glyph/
raster checks that could otherwise permit collateral text changes.

Evidence: `artifacts/mixed-paragraph-publication-20261023/verification.json`.
Corrected focused run: 47 passing native tests. Final expanded publication suite:
eight passing tests; 48 independent actual-export cases. No unresolved review
findings. Exact snapshots and verifier corrections are recorded in
`artifacts/mixed-paragraph-publication-review/acceptance-notes.md`.
