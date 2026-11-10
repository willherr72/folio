# Affine form text editing implementation plan

> **For agentic workers:** Use superpowers:executing-plans inline for this plan.

**Goal:** Edit supported text inside rotated, skewed and mirrored nested/shared
PDF forms while retaining the original transforms, other copies and clipping.

**Architecture:** Retain the stream/resource-chain edit and existing fidelity
proof. Generalize the current positive axis-aligned transform arithmetic. Keep
each ancestor BBox in its local coordinate system, with its composed inverse
transform; test all four PDFium text-bound corners there. Published operators and
matrices stay unchanged. UI uses its existing displayed axis-aligned hit bounds.

**Tech stack:** Rust, lopdf, PDFium/pdfium-render; React/Tauri unchanged.
**Spec:** The design and acceptance requirements below are the binding spec.

## Design and decisions

User authorization: continuous autonomous work toward everyday editor quality;
no repeated design approval. This is a compatibility extension, not a transform
editing tool. Previous cycle completed the explicit form font chooser. Baseline
manifest: `artifacts/form-font-20261023/verification.json` (492 native/749 UI).

Options considered: keep positive-only support; use transformed axis-aligned
ancestor envelopes; or validate native quadrilaterals against actual local BBoxes.
Choose the latter. Envelopes can admit text outside a skewed/rotated clip. Testing
an inverse-transformed run envelope can instead reject fitting rotated text.
PDFium's existing object.bounds() returns its rotated four-corner bounds, so no
new native API or public data model is required.

Support bounded nonsingular affine matrices in page/form cm, form Matrix, and text
Tm. Compose in PDF order, including off-diagonal terms and translation. Retain
per-component finite/1,000,000 coordinate bounds, 100 linear-coefficient cap and
minimum singular value above 0.0001, generalizing the old positive axis-scale
bound. Final native text snapshots retain their existing determinant check.
Keep all graph/encoding/Latin/text-state/source-policy checks unchanged.

Ancestor boxes are fixed at form entry (later cm changes text, not the clip).
Compute inverse coordinates in f64; compare each quadrilateral vertex to the
local BBox. Convert the existing 0.02pt page-space tolerance through each inverse
row norm so scaling/reflection cannot magnify the allowed physical clipping.
This checks a convex quadrilateral inside each convex rectangular clip. Page
edges and neighbor collisions keep their existing conservative page-space
envelopes; flattening and actual no-op still require exact raster/glyph agreement.

Do not modify previously accepted artifacts, the private quiz or user processes.
No external publication/commit/PR. One Cargo command at a time using the shared
target directory and single build job. One scoped independent code review.

## Tasks

### 1. Native reference and refusal fixtures

Create `src-tauri/tests/form_affine.rs` and, if useful, a shared fixture helper.
Author original/reference PDFs with nested shared text and one changed private
leaf. Cover quarter-turn/arbitrary rotation, shear, reflection, nonuniform scale,
composed form Matrix plus cm plus Tm, page crops/rotations, standard and embedded
fonts. Compare exact reference raster and physical glyph geometry after no-op,
longer/shorter edit, export/reopen and repeat edit; other copies/pages unchanged.

- [x] Write tests; run the new native suite and observe the current transform refusal.
- [x] Add a case fitting a rotated clip whose axis-aligned run envelope does not;
  add a converse outside-clip case contained by the clip's axis-aligned envelope.
- [x] Refuse singular, excessive/invalid composed transforms, crossing any ancestor
  edge after longer text, and newly overlapping text; preserve source bytes.
- [x] Cover explicit font substitution inside an affine occurrence, retaining its
  matrix and surrounding text after font registry release and export/reopen.

### 2. Geometry implementation

Modify `src-tauri/src/form_text.rs`; extract only affine helpers into
`src-tauri/src/form_geometry.rs` if needed for a focused geometry unit.

- [x] Replace diagonal-only composition/validation with bounded affine arithmetic.
- [x] Store local BBox plus inverse transform and validate native quad corners.
- [x] Run form/embedded/form-font/positioned-text/protected suites; retain existing
  assertions and repair only tests whose documented capability changed.
- [x] Generate native original/reference/exported artifacts and independently
  verify exact MuPDF glyph/raster comparisons and pypdf text/resource semantics.

### 3. Windows checkpoint and review

- [x] Scoped review checks matrix order, clip-time state, inverse/tolerance math,
  quad ordering and transformed font substitution. Resolve material findings.
- [x] Build and run the isolated Windows editor against rotated/skewed fixtures;
  select the actual hit targets, edit, undo/redo, save/reopen/re-edit. Include a
  missing-glyph substitution. Independently inspect exported PDFs and screenshots.
- [x] Run appropriate regression/typecheck/build gates, record immutable source
  fingerprint/exe/verification manifest, confirm cleanup and preserved hashes.
- [x] Update text-editing docs and product-quality ledger with exact limitations.

## Review focus / ledger

Five priority cases: noncommuting cm/Matrix composition; nested clips with different
axes; reflection changing corner orientation; almost-singular or huge transforms;
replacement crossing a rotated clip despite fitting its world-axis envelope.
Each has a native reference/refusal test above. No parity claim follows this cycle.

Ruling: use minimum singular value, not a raw determinant cutoff, for individual
form matrices. The latter rejected an already-supported 0.0002 uniform scale
compensated by an outer scale of 100. Confirmed failing regression is in
`artifacts/form-affine-small-scale-red-confirmed.log`; the prior attempt used the
filename instead of the Rust module name as filter and ran zero tests.

Accepted checkpoint: full native 501 passed / four opt-in ignores across 52
summaries. Frontend source/dependencies unchanged; previous 749-test result retained
explicitly. Fresh TypeScript/Vite/debug build, five Windows workflows and independent
reader checks pass. Native evidence covers 92 exact reference PDFs and eight font
substitutions. Six screenshots inspected and three owned process records closed.
Review found no demonstrated correctness defects; boundary-coverage opportunities
remain recorded in `artifacts/form-affine-review/acceptance-notes.md`.
Manifest: `artifacts/form-affine-20261023/verification.json`.
