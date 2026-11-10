# Verified embedded fonts in form occurrences

> **For agentic workers:** Use superpowers:executing-plans for inline execution.

**Goal:** Edit supported Latin text inside a shared/nested form using its embedded
TrueType font, changing only the selected occurrence and preserving the document.

**Architecture:** Reuse FontProofs and VerifiedFont for indirect TrueType/Type0
resources, without expanding accepted font mappings. Store bounded encoded show
bytes in the form model; decode and check them against the actual native run using
its verified proof. Encode replacement Unicode back through that exact proof.
Clone the same private resource/stream chain already used by standard-font forms.
No-op cloning uses original encoded bytes and retains whole-page glyph/raster proof.

**Tech stack:** Existing Rust, lopdf, PDFium, React/TypeScript. No dependencies.
This scoped plan is the design. User authorized autonomous routine design choices.

## Decisions and limits

- Keep the existing isolated worktree, source PDFs and prior executable/package
  artifacts. No commits, publication or external messages in this cycle.
- Prefer existing font verification plus existing chain cloning. Converting forms
  into page text would change document structure; font substitution would require
  a separate resource-installation feature and is deferred.
- Accept the same verified Latin character/mapping domain as ordinary embedded
  text. Keep the bounded single Tj grammar, positive axis transforms, ancestor
  boxes, collision checks, text-only source and annotation/tag/transparency refusals.
- Retain 4096-byte decoded-text limit; cap source encoded Tj data at 8192 bytes
  before native decoding. Standard fonts still require printable ASCII. No leading
  or trailing whitespace. Existing UI limit of 1000 characters stays unchanged.
- FontProofs bounds font-data inspection before snapshot calls font.data(). An
  embedded font must have a successful proof for both its indirect resource and
  its actual native name/program identity. Ambiguous identities remain refused.
- FormTextRun exposes isEmbedded with backward-compatible default and existing
  readable display name. UI explains subset limits; no unavailable substitution
  control or misleading substitution advice for this occurrence.

## Implementation and verification

- [x] RED native fixtures: actual simple TrueType subset and two-byte CID subset;
  shared nested placements, inherited/explicit leaf font resources, rotations,
  independent reference PDFs, export/reopen, exact original font bytes and source.
  Refuse missing glyphs, malformed/ambiguous font maps, odd CID bytes and overflow.
- [x] Implement bounded encoded occurrences and proof-gated font resources in
  `src-tauri/src/form_text.rs`; reuse existing encoding, clipping, pixel and
  glyph checks. Add DTO `isEmbedded` in `types.rs`; existing frontend spread keeps it.
- [x] Add focused UI regression for readable embedded form label, subset guidance,
  Unicode request and absent substitution/group/paragraph controls. Update help.
- [x] Run native form/embedded/group/paragraph/protected gates and frontend gate.
  Request one scoped fresh review; reproduce and correct material findings.
- [x] Build isolated Windows checkpoint. Edit one accented shared occurrence,
  undo/redo, save/reopen; try a missing glyph and retain draft/source. Independent
  readers compare exported text, raw glyph geometry, pixels and embedded font
  bytes against separately authored reference; inspect screenshots and cleanup.
  Preserve prior artifacts, record hashes/limits and update quality record.

## Files and review focus

Production: form_text.rs, types.rs, ExistingTextDialog.tsx. Tests:
`src-tauri/tests/embedded_form_text.rs`, `src-tauri/tests/support/embedded_forms.rs`,
`tests/existing-text-dialog.test.tsx`. Dedicated desktop smoke and independent
verifier build on existing Windows/WebView workflow.

Review raw versus Unicode strings; proof tied to actual resource/program; alias
ambiguity; inherited leaf scope and shared copies; no-op bytes; bounded font decode;
missing-glyph feedback; unrelated operators/glyphs and original source preservation.

## Import fidelity prerequisite discovered during RED

The original fixture exposed lost /First resources after native page import and
save. This is not alias ambiguity: PDFium CPDF_PageOrganizer::UpdateReference
skips First/Prev/Parent in every dictionary. Raw native reproduction and upstream
source are retained under artifacts/embedded-form-red-fixtures and
artifacts/cpdf_pageorganizer.cpp. A separately scoped export regression failed
before the fix. `page_resources.rs` now detects these names in selected source
resource graphs and reconstructs affected graphs with fresh destination IDs,
preserving names, content bytes and intra-page aliases. It runs before imported
pages are loaded or overlays are applied in composition and editing isolation.
Ordinary pages avoid the extra destination serialization/reload. Source graph
inspection and copied bytes are bounded; document-page-tree resource references
are refused. No guessing from dangling IDs and no content rewriting.

Additional scope: page_resources.rs, engine.rs composition order, text_edit.rs
editable_page_copy, lib.rs, resource_name_copy.rs. Expanded regression covers
all three reserved names, shared and unique forms, nested forms, font resources,
rotation, duplicated/reordered pages, exact pixels/text and original font bytes.
Full native/frontend gates passed because composition affects all export. Review
identified obsolete imported graphs consuming the intermediate writer budget;
the reproduced regression now passes after bounded trailer-rooted pruning. An
indirect page-tree type guard and duplicate error prefix were also corrected.

## Ledger

Previous goal turn: progress, verified uniform paragraph color checkpoint.
Baseline: `artifacts/paragraph-color-verified-20261023/verification.json`.
Final: 488 native tests, four existing opt-in ignores; 740 frontend tests;
TypeScript/Vite/debug build; owned Windows workflow and MuPDF/pypdf comparisons.
Manifest: `artifacts/embedded-form-20261023/verification.json`.
