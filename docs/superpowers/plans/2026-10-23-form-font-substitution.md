# Explicit form-occurrence font substitution

> **For agentic workers:** Use superpowers:executing-plans for inline execution.

**Goal:** Choose an installed/imported full font when a supported shared/nested
form occurrence's original font cannot supply the replacement characters.

**Architecture:** Reuse the existing FontPicker, immutable FontAsset registry and
audited full Type0 font installer. Extend the existing private form-chain clone:
install a unique font alias in the selected leaf scope and replace its active Tf
name plus encoded Tj, retaining size, transform and color. The native post-save
proof requires the requested program hash and unchanged layout style for the
selected run, with all other occurrences unchanged. Original-byte no-op isolation
remains mandatory. Published output embeds the font and survives registry release.

**Tech stack:** Current Rust/PDFium/lopdf and React/TypeScript; no dependencies.
**Spec:** The design and acceptance contract below are the scoped specification.

## Design decisions and constraints

User authorized autonomous implementation; routine choices do not need another
approval. Previous goal turn was progress: embedded-form font reuse and import
resource-name preservation verified in `artifacts/embedded-form-20261023`.
Retain the current isolated worktree and all earlier artifacts/source PDFs. No
commits, PR, publication or external messages in this cycle.

- Reuse the existing resource/stream clone. Native shared-object mutation would
  require separate ownership isolation; converting the form into page text would
  discard its structure. Installing a local resource keeps the proven path.
- Eligibility stays limited to supported, unambiguous source runs. No font chooser
  for unknown source encodings. Keep Latin/4096 UTF-8 byte native text and 1000 UI
  character limits, positive form transforms, ancestor/page bounds, overlap,
  tagged/annotation/graphics-state refusals, and source edit permissions.
- Font choice is explicit. No silent fallback. Cancel/reset retains the text
  draft. A glyph/fit failure retains both draft and chosen font for correction.
- Choose a unique leaf font name. Change only the last active Tf inside the
  selected BT/Tj block; every later text block already requires an explicit font.
  For an inherited leaf, materialize its effective resources in that private
  clone; source admission already refuses missing scopes on intermediate forms.
  A selected node can itself have descendants: retain their original font aliases.
- Original font omission keeps old APIs/IPC compatible. Add
  `replace_form_text_with_font(..., font_id: Option<&str>)`; the existing public
  `replace_form_text(...)` delegates with None. The worker and command carry the
  optional ID. FormTextRun exposes defaulted canSubstitute. TS carries the flag
  through ExistingTextLayer and only offers substitution when true.
- Selected output must match FontAsset.info.id (actual program SHA), size, color,
  render mode and matrix. Every other run keeps exact font/program/style/text.
  Reopen and re-edit using the new embedded font without registry/file access.
- Same-text font changes are permitted. Full registry validation/embedding
  permissions apply. Bounds failures leave all source/history/registry state intact.

## Tasks and acceptance

### 1. Native occurrence substitution

Files: `src-tauri/src/form_text.rs`, `engine.rs`, `types.rs`, `app.rs`;
tests `src-tauri/tests/form_font.rs`, existing `support/embedded_forms.rs`.

- [x] Add failing tests for choosing a full DejaVu font to replace Café with Cafè
  in simple/CID shared nested forms. Cover explicit/inherited leaf scope, all
  rotations, unchanged other copies and pages, source bytes and original fonts.
  Cover a leaf with two text runs so the neighboring run retains its original Tf.
  Same-text font changes work; missing IDs/glyphs, stale identity and oversized
  replacement fail without modifying source. The chosen full font survives
  registry release, export/reopen and subsequent same-font edits.
- [x] Add the backward-compatible public/worker/command API and native flag.
  Extend Model/clone_chain with optional installed resource. Validate actual
  output program plus same_layout_style; retain all previous fidelity checks.
- [x] Run the new suite and existing form/embedded/protected/group/paragraph
  suites. Independently compare selected glyph origin/style, original shared
  copies, outside-area pixels and exact embedded program bytes.

### 2. Font chooser and desktop flow

Files: `src/editor/{types,adapter}.ts`, `src/components/ExistingTextLayer.tsx`,
`ExistingTextDialog.tsx`, `src/App.tsx`; tests `form-text-app.test.tsx`,
`existing-text-dialog.test.tsx`, `text-edit-adapter.test.ts`, and focused form
font dialog tests if a mocked picker makes state transitions clearer.

- [x] RED: adapter omits absent IDs and forwards selected ones with the complete
  path. Dialog offers chooser only with canSubstitute, keeps grouping/paragraph
  controls hidden, supports same-text font change, reset/cancel and error drafts.
  App routes the chosen ID only to form replacement, records one undo step, leaves
  duplicate pages untouched, and cleans a late derived source after unmount.
- [x] Wire existing picker/preview and optional IPC. Preserve older form metadata
  without the flag and remove unavailable-substitution wording only when allowed.
- [x] Run full frontend, native suite and typecheck/build. One fresh scoped review
  checks font scope/identity, sibling state, ownership, stale responses and bounds.
  Reproduce and fix material findings before acceptance.
- [x] Build an isolated Windows checkpoint; start with a missing-glyph error,
  choose/import font, apply, undo/redo, save/reopen and re-edit with the embedded
  font after release. Check a narrow dialog and error/cancel/reset flows. MuPDF
  and pypdf verify output fonts/text/geometry and untouched shared copies. Inspect
  screenshots; close owned processes; preserve earlier hashes; record manifest,
  limitations, user-facing docs and the product-quality ledger.

## Review focus

Five priority cases: an inherited leaf with neighboring text; multiple Tf before
one Tj; font-name collision in effective resources; a failed/late apply while the
picker owns a font; and choosing the same program already embedded in the source.
The native fixtures cover scope/Tf/collision/same-program identity; UI tests cover
draft/late-response ownership. Missing glyphs and bounds remain real output checks.

## Ledger

Baseline native488 (four opt-ins ignored), frontend740, verified Windows output.
Baseline executable SHA: e0035f44a142463f53ff4fae6b5fec076f4ebaad2fc6b773c82ea6cc1ab13a72.
Ruling: a selected text node may also contain Do children. Keep original aliases
for those descendants; test substitution in an intermediate form with an inherited
child. When the chosen program already matches and its proof encodes the new text,
reuse that exact verified encoding/resource to avoid accumulating full programs.
Otherwise remove an obsolete selected font alias only in a childless node, only
when no remaining Tf refers to it. This preserves scope and bounds repeat edits.
Initial native regression fails compilation because the new method is absent,
as recorded in `artifacts/form-font-native-red.log`.

Accepted evidence: 492 native tests passed (four existing opt-ins ignored),
749 frontend tests across 106 files, typecheck/Vite/debug build. Review found no
confirmed actionable defects; its failure-lifecycle gap now has two passing app
regressions and actual Windows import/failure/retry coverage. Original same-program
non-identity map reuse and repeated distinct-font switching in a parent with
inherited descendants remain unmeasured cases, explicitly recorded in the review
response. The latter conservatively retains aliases; no no-growth claim is made.

Both Windows encoding cases pass missing-glyph recovery, imported preview,
cancel/reset, failed substitute retry, undo/redo, save/reopen and re-edit without
the font file or registry entry. MuPDF/fontTools/pypdf confirm chosen program,
glyph IDs, stored widths, baseline/size/color, untouched shared copies/pages and
outside-area pixels. MuPDF's observed integer-width extraction rounding is modeled
separately from checking exact fractional PDF widths. pypdf omits original inherited
leaf text in these fixtures; MuPDF verifies those glyphs and pixels directly.
Four screenshots inspected; all three owned process records closed.
Checkpoint: `artifacts/form-font-20261023/verification.json`.
