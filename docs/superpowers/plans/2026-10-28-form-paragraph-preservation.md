# Form Paragraph Preservation Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline. Autonomous
> execution is already authorized; one final scoped review follows acceptance.

**Goal:** Admit faithful horizontal nested paragraph edits by fixing inconsistent
renderer separator geometry, without weakening Folio's preservation checks.

**Architecture:** Normalize horizontal line-overlap comparisons into one Form
coordinate system in the locked PDFium source. Add a reproducible patch and probe,
then verify the candidate dependency through existing native and user workflows.

**Tech Stack:** PDFium C++, Rust engine, Python ctypes/PyMuPDF/pypdf, Windows build.

**Spec:** `docs/superpowers/specs/2026-10-28-form-paragraph-preservation-design.md`

## Global constraints

Preserve accepted binaries/originals/all failed evidence. One Cargo, jobs1,
locked/offline and serial tests, existing shared target directory. No Rust proof
relaxation; no installed dependency adoption before candidate checks. Positive
axis-aligned horizontal Form cases only; other matrix/writing-mode behavior stays
unchanged. Record expected generated-separator changes honestly. No commits,
cleanup, release or commercial-parity claim.

## Review focus

1. Same visual line across differently translated/scaled Forms must remain a line
   (Task1 shared/nested/root controls).
2. Tiny/degenerate/nonfinite transform values must not lead to new invalid inverse
   geometry or broad acceptance (Task1 matrix admission controls).
3. Vertical/reflected/rotated/sheared Forms and same-transform paths must retain
   baseline behavior (Task1 differential controls, Task2 native regression).
4. Existing renderer fixes and provenance must survive dependency adoption
   (Task2 six renderer probes plus complete fingerprint/lock checks).
5. No-op and changed paragraph saves must preserve source ownership, unaffected
   text/pixels and subsequent editing (Task3 CLI/MCP/desktop acceptance).

## Task 1: reproduce and prove the renderer correction

**Files:** `src-tauri/src/form_paragraph_inherited_tests.rs`, test module declaration
in `form_paragraph_projection.rs`; artifacts under
`artifacts/form-paragraph-preservation-20261028/`; eventual
`scripts/probe-pdfium-form-separators.py` and
`patches/pdfium/form-separator-coordinates.patch`.

**Interfaces:** Native `with_form_paragraph_projection` and `Model::promote_paragraph`
remain unchanged. Probe accepts `--dll PATH --output NEW_DIRECTORY`, emits exact
input/output/renderer fingerprints and individual comparison outcomes.

- [x] Capture baseline source copies and reproduce complete-glyph-only mismatch
  with exact raster/physical-glyph equality. Retain the failing Rust test/log.
- [x] Build a disposable candidate normalizing incoming horizontal rectangle and
  position into preceding Form space; restore cached source after the experiment.
- [x] Compare source/promoted extraction and rendering directly and run 24 positive
  synthetic geometry controls, retaining baseline and candidate outcomes.
- [x] Complete focused Rust candidate test; do not interpret a candidate-only pass
  as a passing default suite while the old renderer remains installed.
- [x] Extend the standalone probe with mixed page/Form, actual same-line spacing,
  unequal scales, tiny/singular limits, negative/sheared/rotated/vertical controls,
  repeated aliases and nested Forms. Require literals for expected text where
  specified and unchanged candidate/baseline behavior for excluded classes.
- [x] Check finite/inverse safety; refine the patch only on reproduced failures.
  Preserve each unsuccessful candidate and rerun affected controls.
- [x] Promote the final probe/patch into scripts/patches with bounded inputs and
  no test-only code in production. Preserve all original experimental evidence.

## Task 2: integrate the locked dependency and run native regression

**Files:** `scripts/pdfium-source-lock.json`, `scripts/build-pdfium-source.py`,
`scripts/build-fingerprint.py`, `scripts/build-portable.ps1`,
`scripts/fetch-pdfium.ps1`, `src-tauri/resources/pdfium/provenance.json` and DLL;
only additional lock/provenance consumers found by a targeted search.

**Interfaces:** Add `separatorPatch`/`separatorPatchSha256` and source hashes using
the existing patch convention. Build a new renderer variant, preserve notices,
and bind every accepted binary to its exact patch/source/toolchain inputs.

- [x] Complete six existing renderer probes (inheritance, extended advances,
  AllCap, ordered colors, embedded Form fonts, fractional borders) on the candidate.
- [x] Update all identified provenance/lock/build consumers, create a new separate
  reproducible build, and compare it with the tested correction before adoption.
- [x] Preserve installed baseline bytes/provenance, then install the verified
  candidate in the development workspace only. No accepted package replacement.
- [x] Run focused paragraph projection/form/nested automation suites, then the
  full ordinary native suite with failure continuation and current source/test/
  renderer fingerprints. Investigate every failure; update prior refusal assertions
  only when fresh complete-glyph/raster evidence proves the case now faithful.
- [x] Confirm ordinary page extraction, glyph coordinates, security restrictions,
  existing no-op appearance and cleanup tests still pass. Keep ignored opt-in gates
  explicit; the newly reproduced no-op byte/selector identity failures belong to
  Task3 and must not be described as passing this baseline gate.

## Task 3: user workflow acceptance and final checkpoint

**Files:** artifact harnesses; `docs/product-quality-loop.md`, `docs/automation.md`,
this plan and the execution ledger.

**Interfaces:** Existing native/CLI/MCP routes; no new client request fields.

No-op correction files: `src-tauri/src/automation.rs`,
`automation/text_preview.rs`, `automation/paragraph_preview.rs`, and the internal
preview description in `automation/form_automation.rs`; private bounded snapshot
ownership in new `automation/original_snapshot.rs`; native regression additions
in `tests/text_automation.rs` and `tests/paragraph_automation.rs`. Keep the existing
publication/validation implementation intact except for exact no-op candidate bytes.

- [x] Reproduce all eight page/nested, text/paragraph, CLI/MCP exact-byte failures
  with `artifacts/form-paragraph-preservation-20261028/accept-no-op-identity.py`.
  Expected RED: eight assertion failures for byte identity, zero harness errors.
- [x] After the full renderer gate is terminal, add failing tests for protected
  unchanged CLI copies and source snapshot/mutation/lifetime semantics. Preserve
  the existing eight-case RED harness and its exact hash before any extension.
- [x] Add `OriginalSnapshot::capture(&Path) -> EngineResult<Self>` and `path()`
  around a private NamedTempFile plus bounded FileProof in original_snapshot.rs.
  Capture only PreviewText/PreviewParagraph inputs before engine opening in
  execute; pass the retained snapshot to their preview methods. Native validation
  must still run before publication. A derived unchanged flag chooses the
  snapshot bytes only for exact genuine text or equivalent ordered character/style
  pairs and unchanged layout. Empty/split equal-style runs are equivalent; echoed
  bounds must compare exactly, and explicit alignment/spacing require a known
  equal source value. fit:true remains an explicit layout request.
  Hash-check the copied candidate, preserve encryption, and never put the borrowed
  original DocumentInfo into an owning Source guard. Match the existing Windows
  MCP input's kernel-owned delete-on-close lifetime; verify a worker exit without
  destructors leaves no private input. Form preview uses no snapshot.
- [x] Run snapshot/detection tests and text/paragraph/nested/CLI/MCP/form automation
  suites with serial locked/offline Cargo and exact fingerprints. Expected GREEN:
  zero failures; explicit style/layout changes and denied protected edits must
  still take their normal paths. Broaden only for new failures/changed dependencies.
- [x] Run the unchanged eight-case acceptance harness against the fresh CLI.
  Expected GREEN: eight passes, original byte/hash identity, same-selector reuse,
  normal changed-candidate bytes/pixels and independent MCP document lifetimes.
- [x] Run `accept-protected-no-op.py` and `accept-no-op-run-equivalence.py`
  against that same fresh CLI. Expected GREEN: four exact encrypted-copy cases
  preserving reader restrictions, and three equivalent-run/echoed-bounds cases.
  Retain their baseline RED results and exact harnesses alongside the eight-case
  unprotected transport regression. Add unit controls for unknown layout values,
  different style IDs, visible text changes, Unicode run boundaries and fit:true.

- [x] Build fresh desktop/CLI candidates. Reuse frontend evidence only if frontend
  inputs are unchanged, and disclose that scope.
- [x] Verify newly admitted paragraphs through no-op and added-text saves, exact
  candidate hashes, immutable prior documents, stale selectors, reinspection,
  re-edit, and MCP Apply/Save. Resolve the prior nested no-op acceptance gap with
  explicit evidence; live preview cancellation remains a separate carried gap.
- [x] Independently check saved text/glyph positions/pixels, unselected pages and
  both other invocations and graph content. Retain the known pypdf inherited-text
  omission rather than inventing a second reader edit witness.
- [x] Exercise native desktop edit/Preview/Apply/Undo/Redo/save/reopen on synthetic
  newly admitted input; stop only the exact owned process identity.
- [x] Obtain one final scoped review after acceptance. Resolve demonstrated
  findings inline, retain coverage limits, verify fingerprints and preservation,
  then record a local checkpoint and the next real product gap.

Self-review: the reproduction/correction maps to Task1; dependency reproducibility
and regression to Task2; actual editing and output fidelity to Task3. All five
review-focus classes have owning steps. The user already opted out of routine
approval questions and inline execution remains the chosen workflow.
