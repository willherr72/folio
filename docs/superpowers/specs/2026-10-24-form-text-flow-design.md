# Font and line flow inside reusable PDF text

## Intent and observed failure

Ordinary ReportLab 4.5.1 `beginForm`/`drawString` and `PDFTextObject.textLine`
output is readable but entirely uneditable in the accepted positioned-form build.
The preserved three probes under `artifacts/form-text-flow-20261024/probe` fail
at an empty BT/Tf/TL/ET setup object before any glyph is inspected. Later shows
reuse that font and use T* or multiple explicit line positions. This is an existing
document compatibility milestone, not paragraph reflow or a new UI.

User standing authorization covers autonomous implementation and verification.
Use the existing Rust/lopdf/PDFium pipeline and owned Windows workflow. Preserve
the accepted positioned-form executable and dirty worktree; no commits, removal,
source-document overwrites or external publication.

## Approaches and choice

1. Normalize the published form into separate text objects: easier mapping, but
   changes original operators and risks line/state fidelity. Reject this approach.
2. Track the actual font and line-position dependencies while retaining original
   operators, then isolate a selected substitute font with local select/restore
   instructions. Chosen by the coordinator: extends the existing verified clone
   path without moving unselected lines or flattening published pages.
3. Reconstruct every glyph advance and compensate arbitrary adjacent shows:
   valuable later, but a separate typography dependency. This milestone covers
   independently positioned shows; adjacent shows whose origin depends on the
   preceding edited advance stay explicitly unsupported.

## Required behavior

- Balanced empty BT/ET objects may change font and other text state without
  producing selectable objects. Tf persists across BT/ET, q/Q and enclosing forms
  according to graphics state. A font resource is resolved at its Tf, not again
  under a child's potentially shadowing resource names. No font defaults are invented.
- Tm, Td, TD and T* are supported inside BT/ET. TD also sets leading to negative
  ty. Multiple glyph shows can share one BT/ET when each show has an explicit
  position independent of prior glyph advance. Tm/Td/TD/T* reset that dependency;
  numeric-only TJ does not. Existing Tc/Tw/Tz/Ts/TL rules remain intact.
- Single/double quote shows are supported as line-moving shows. The double quote
  sets word and character spacing before its line move and show; these values
  persist afterwards. Preserve the quote operator and operands during publication.
- Each Tj/TJ/quote glyph show retains a separate stable object path only where
  native object count/type and source encoding prove that mapping. No grouping or
  guessed whitespace. Native merging, empty glyph strings and malformed syntax
  remain refusals. BT/ET and T* require zero operands; other existing bounds apply.
- Every occurrence records its effective verified font resource and original size.
  Represent shared font selections through reference-counted storage so a direct
  font graph is not cloned per occurrence. Bound actual resource copies using the
  existing CopyBudget.
- Installing a font inserts Tf immediately before only the selected show and a
  restoring Tf immediately after it. The restore resource must denote the exact
  previously selected font, including when selected in an ancestor with a shadowed
  name. Use fresh aliases, retain existing entries, and materialize a missing leaf
  resource scope only through the existing approved resource chain rules. Do not
  rewrite the earlier setup Tf, which may be shared by other text.
- Same-font edits leave state operators intact. Source TJ slot rewriting is
  unchanged. New-font single-byte nonzero Tw restrictions use effective state at
  the show, including double quotes; exact verified source-program reuse remains.
- Keep original/private-flat, actual no-op, all glyph/style/native mapping,
  ancestor box, collision and outside-region raster checks. Keep shared copies,
  other lines/pages, protections and immutable undo/recovery source behavior.

Text-state persistence and positioning follow ISO 32000-1:2008 sections 9.3/9.4,
tables 105/107–109 ([Adobe specification](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf)).
The implementation can preserve text-state Tf outside BT/ET; positioning/show
operators require a text object. The existing prohibition on q/Q/cm inside BT/ET
remains. Broad graphics, annotation, complex-script and reflow support are separate.

## Acceptance

Use the real ReportLab probes plus independently authored Rust fixtures with
standard/simple/CID fonts. Verify first and later lines, original/no-op/shorter/
longer text, quote state, TD leading, inherited and restored fonts (including same
resource name with different font), local alias collisions, numeric cursor arrays,
malformed syntax and dependent adjacent-show refusal. Explicit new-font edits must
leave subsequent lines and shared copies pixel-identical and re-editable. Include
scaled/rotated/sheared cases, clipping/collision refusal, protected output and
recovery. Preserve each original and compare exports against authored references
with MuPDF/pypdf, including text, geometry and pixels.

Run relevant native suites and TypeScript/debug build, one fresh scoped final
review, and owned Windows hit-target/edit/font-choice/undo/redo/save/reopen checks.
Record hashes, diagnostics and coverage limits. No frontend source changes are
planned; prior frontend suite evidence is carried only if hashes remain unchanged.

## Builder choices

Internal helper layout and exact test factoring are coordinator choices. Use a
small form-flow helper for text-object/line-state admission and quote decoding;
keep resource proof, traversal and native publication in form_text. Do not introduce
new dependencies, controls or global normalization. Broader parity remains active.
