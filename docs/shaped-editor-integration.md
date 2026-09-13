# Shaped text-box integration

Released in v0.10.0 as the supported subset of #14.
Shaping is a default native build feature; the new editing
path is explicitly selected per text box. Older boxes keep their legacy behavior.

## Using it

Create or select a text box and enable **Shaped text** in Properties. Enter your
single line, then use **More fonts…** to choose an installed or imported static
TrueType-outline font. The picker validates the actual text at the authored size
before enabling **Apply font**. Direction defaults to Automatic; ligatures can
be toggled. Missing glyphs and unsupported layouts display an explanation.
**Retry preview** retries failed preparation, including after freeing cache space.

The canvas draws native glyph paths. The standard Content field provides typing,
selection, copying and platform caret/deletion behavior. IME composition remains
local until compositionend, then becomes one committed workspace edit. Switching
boxes/tabs discards obsolete composition; save/print/modal busy state disables
controls and resets the local draft so it cannot diverge from saved data.
Native errors retain committed logical text for correction and recovery.

Search uses exact logical Unicode and native cluster rectangles, including the
shared rectangle of a ligature or combining sequence. Direct dragging of an
editable annotation still moves/selects the box; glyph-level canvas selection
of that editable box has not been added. Its text can be copied in Content;
flattened page text uses the existing PDF text selection/highlight layer.

## Stored model and shared appearance

The optional text-overlay field is
`shaping: {version:1, direction:"auto"|"ltr"|"rtl", ligatures:boolean}`.
Absence means legacy rendering; standard-font selection clears shaping. An exact
custom font ID is required for shaped export. Immutable options are retained
through document history and page duplication.

`prepare_text_overlay` holds an Arc to the registered font before background
work. It returns local bounds/logical character rectangles and the native vector
preview, with the semantic baseline origin. The frontend maps those source PDF
axes to the existing top-left text-box baseline and rotation. Preview geometry
never uses approximate browser text advances for shaped text.

Save regenerates the same native representation from text/options/font bytes,
then transforms it into the page's crop/rotation coordinate frame. Portable
metadata version 3 retains editable controls; the original font is embedded.
On reopen, the full appearance content, resources and font are regenerated and
compared before importing metadata. Altered records or semantic resources stay
native PDF content instead of being accepted as editable controls. Flattening
paints the same appearance as page content. Recovery retains logical text/options
and exact font snapshots; glyph arrays are not trusted persisted input.

## Ownership and bounds

The frontend shares native work by font/text/size/color/shaping, independent of
box placement and ID. One request runs at a time; abandoned queued work is
removed, and an in-flight request retains its font until it settles. A request
for older text cannot replace a newer result. Search consumes the immutable
response directly so eviction cannot remove already computed hits.

Preview storage admits at most 64 entries and 32 MiB of estimated serialized
UTF-16 payload. It evicts unused results before rejecting new work. Active
consumers remain valid; admission/memory failures have an explicit retry action.
Existing native font, shaping, outline and serialized-output limits remain.
Font character-map preflight bounds enumerated ranges, callback work and linear
lookup costs, including malformed formats 2, 12 and 13.

## Evidence

- **64/64** actual editor-versus-flattened-PDFium raster comparisons: eight samples,
  four overlay rotations, 150%/300% zoom. Maximum differences: ink bounds 1 px,
  weighted centroid 0.580 px, weighted ink area 4.165%, unmatched ink 0% within
  the declared 2 px neighborhood. Fixed tolerances were unchanged. The first
  harness run exposed fractional HTML positioning adding one screenshot row;
  integer fixture layout corrected that capture artifact before comparison.
- Native editable save/reopen/flatten tests cover ligatures, two-axis marks,
  Arabic, Indic and empty text at all four overlay rotations, source-font
  retention, exact scalar order and requested raster color. Native cluster
  rectangles agree with PDFium within 0.05 pt, allowing its 1/1000-em metric
  quantization at 24 pt.
- A further **32-case** matrix covers nonzero asymmetric crop origins, every source/overlay quarter turn, and page-plan rotation. Restored controls, text order, cluster geometry and raster ink bounds pass.
- Unsupported multiline drafts survive recovery unchanged while native preparation and export continue to refuse them.
- Tests reject modified shaping metadata and semantic resources. Exact-font
  recovery continues after original source/font deletion.
- Frontend regressions cover composition boundaries, late responses, duplicate
  preparation, busy-state draft protection, actual-size font validation, search
  beyond cache capacity, font leases, memory eviction and retry.
- Independent review found and verified fixes for search eviction, preview
  admission, busy draft divergence, wrong-size picker validation and cmap work.

Measured browser cases are in [shaped-editor-verification.json](shaped-editor-verification.json).
Reproduce native assets with `cargo run -j 1 --offline --locked --manifest-path
src-tauri/Cargo.toml --example shaped-editor-probe` (output directory must not
exist). Start Vite on 1427, then run `node scripts/smoke-shaped-editor-browser.mjs`.
The native generator and browser harness write beneath `artifacts/shaped-text`.

## Remaining limits

Supported evidence is bounded single-line text. Mixed directional runs, RTL word
separators/joiners, whitespace-only lines and more than 255 distinct semantic
character definitions are still refused. Empty boxes are supported. Font
fallback, paragraph wrapping, general existing-PDF complex-text replacement,
variable/color/CFF fonts and glyph-level canvas caret editing are not introduced.
No real-reader Acrobat/Foxit copy/selection or physical OS IME manual acceptance
is claimed. Packaged WebView composition, save/reopen, selection, search and recovery
checks are recorded in [v0.10.0 release verification](verification-v0.10.0.md).

## Final checks

The v0.10.0 release rerun passed all 137 default-feature native tests, with three
existing opt-in checks ignored, and all 241 frontend tests across 48 files. The
TypeScript/Vite/Tauri production build, Rust formatting, whitespace and independent
review passed. See the release verification for packaged desktop acceptance.
