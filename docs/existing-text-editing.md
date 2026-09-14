# Existing PDF text editing

Folio edits existing text objects in supported PDFs. This changes the PDF content itself: exported and recovered pages contain the replacement as searchable text, and reopening an export exposes the edited run normally.

Folio supports visible, filled, top-level single-line text using the twelve Latin standard PDF fonts, plus positively verified embedded TrueType fonts. The standard-font path uses printable ASCII. Embedded simple fonts and Type0 Identity-H CIDFontType2 fonts can reuse supported Latin characters, including precomposed accents, only when their code, glyph and width mappings are unambiguous. Replacement text stays on one line with at most 1,000 characters. Original font, size, color and baseline are retained by default. Page crop offsets and intrinsic quarter-turn rotations are supported.

For an eligible run whose subset lacks a needed character, choose **Choose substitute font…**, preview an installed or imported font, choose **Apply font**, then **Apply changes**. This explicitly changes the font while retaining size, color and baseline. The substitute travels inside the changed source PDF, so export/reopen and recovery do not depend on an installed font or separate imported file. The dialog keeps your typed replacement when a glyph or layout check fails.

Since v0.7.1, replacements can extend beyond the original run into available space while remaining within the visible page. Folio rejects new or expanded overlap with neighboring source text and graphics using conservative object bounds. Only a proven opaque, unstroked rectangular page fill painted behind the run is treated as a background. Images, forms and compound paths remain subject to conservative collision bounds, even when they cover the whole page. It does not reflow a paragraph or shrink the font to make an edit fit. Collision checks cover source page objects; separately placed annotations and overlays still need visual checking when expanding text.

Unverified or ambiguous font mappings, embedded CFF/Type1/Type3 programs, symbolic fonts, multi-character ligatures, and invisible/stroked/clipping text are unsupported. Scans do not produce editable runs. Nested forms have the narrower occurrence-editing support described below. Documents containing optional content (PDF layers) are disabled, because copying a page can lose the catalog's visibility settings. Pages containing clipping paths, unverified custom font encodings, named pattern paints, or advanced graphics state are conservatively disabled as a whole. Explicit substitution additionally refuses marked-content/tagged text. Some remaining encoding or text-state problems can only be identified when applying the replacement; the editor reports the error and keeps the existing page.

## Positioned text (v0.11.0)

Verified single runs can retain rotation, scale, shear, horizontal scale and rise.
Folio accepts finite, nonsingular affine matrices within explicit bounds and uses
page coordinates for fit and conservative collision checks. Existing paragraphs
are not reconstructed or reflowed.

Same-font edits on pages with explicit character/word spacing or numeric `TJ`
adjustments use a uniquely mapped content operator. The original font resource,
text state and every other operator remain in place. Numeric `TJ` gaps stay at
their original character boundaries: a longer replacement extends after the
original slots; shortening discards gaps at or beyond the new end. Leading gaps
remain. A no-op retains the entire original sequence, including trailing gaps.
This preserves the document's specified spacing; it does not infer fresh kerning
for different letter pairs.

Routing is deliberately conservative across the page: even unrelated or later
reset spacing operators require the verified stream path. If PDFium cannot map
text-show operators uniquely, a segment splits an encoded character, or a changed
advance moves a relative neighboring run, the edit is refused. Explicit font
substitution remains unavailable on pages with these spacing operators. The
ordinary native edit path retains its previous neighbor-placement behavior.
Spacing is preserved even when it is dormant in the original string, such as word
spacing before replacing a word with a phrase. Original-font reuse continues to
require positive encoding, glyph and width evidence.

[Reader verification](issue15-reader-verification.json) compares 144 native
exports against independently authored reference PDFs: PDFium glyph geometry and
raster/save-reopen checks, MuPDF text/raster, and pypdf text. The matrix covers nine
text states, four page rotations, and no-op/equal/longer/shorter replacements.
Additional native tests cover leading/trailing/consecutive adjustments, dormant
spacing, multibyte embedded fonts, split streams, shorthand text operators,
resource bounds and refusal without source mutation.

## Edit adjacent pieces together (v0.12.0)

Choose **Edit text**, click a piece, then choose **Edit together…**. Select 2–8
consecutive pieces from the nearby-text list and choose **Check selection**.
When the check succeeds, edit the combined **Replacement text** and apply.
`examples/Edit text together.pdf` provides split words and phrases to practice on.

Membership is explicit. The pieces must use the same underlying PDF font,
size, color and linear transform, and must join without changing any character's
position or the page appearance. Folio checks the encoded characters as well as
the extracted text; a generated space or silently omitted character is a reason
to refuse the group. Identical font names alone do not establish compatibility.

This first grouping stage supports the existing verified, unshaped Latin font
subset. Tagged documents, marked content, explicit spacing, nested forms,
intervening graphics and uncertain reading order remain unsupported for groups.
A group cannot choose a substitute font. Fractional advances that cause even tiny
raster differences can prevent a group from passing. Compatible rotated or transformed pieces
can pass the same preservation proof. Longer replacements can use available space,
subject to the existing crop and collision checks; paragraphs do not reflow.

[Independent reader results](issue15-group-reader-verification.json) cover 32
accepted group exports, with exact MuPDF text/raster and pypdf text comparisons
to separately authored references. Native tests additionally cover embedded fonts,
resource identity, invalid selections, refusal cases and temporary-source cleanup.

Checking a selection leaves the document unchanged. Applying repeats the proof on
a private page copy and creates one undo step. Canceling or changing the selection
invalidates a pending check. Failed edits retain your replacement draft.

## Edit one form occurrence (v0.13.0)

Some PDFs reuse one small content stream in several places. Choose **Edit text**
and click a supported occurrence to edit it with the normal replacement dialog.
The dialog confirms that only this occurrence changes. Folio copies the selected
form/resource chain and redirects only that placement; other copies retain their
original text. `examples/Edit one shared occurrence.pdf` demonstrates two shared
placements on page 1 and another on page 2, nested two form levels deep.

This first stage supports the standard Latin Type1 PDF fonts, printable ASCII
without surrounding whitespace, positive scale/translation and a conservative
text-only content grammar. Explicit character spacing, rotated/sheared form
transforms, embedded/custom fonts, grouping and substitution are not supported
inside forms. Text must fit inside the page and every ancestor form box without
new overlap. Tagged/layered sources, source annotations/appearance streams,
clipping, transparency, images and uncertain inherited state remain refusal cases.
Existing top-level text, added annotations and their font features retain their
previous behavior.

An inherited leaf font resource can be preserved. An intermediate form without
its own resource dictionary is refused: adding a dictionary there can change
lookup semantics. The exact resource chain and invocation path identify the
selection, even when several copies have identical text and font names.

A bounded graph check runs for the new form-editing operations before native page
copy/traversal. It refuses cycles, excessive depth/expanded work and uncertain
inline-image parsing. Direct resource copies have cumulative byte/value limits
checked before allocation. It does not change ordinary PDF opening or claim to harden
all native rendering. A private validation page is flattened only for comparing
font/text geometry; published pages keep their nested forms. Actual no-op cloning
must preserve the full page text/geometry and raster. Changed output is reopened
and checked before the new immutable source becomes available.

## Preservation and history

The backend copies only the selected source page and edits that private copy. It never mutates a previously opened source. Applying the change creates a new one-page source; the frontend retains the workspace page identity, rotation, and annotation overlays while switching its source reference. Other pages and duplicate instances of the old source remain unchanged. Undo and redo select the retained immutable sources.

The original filesystem path is inherited through every edit, preserving the existing protection against overwriting an original PDF. The new source snapshot holds the edited PDF bytes, so printing, export, and recovery use the same changed content. Imported editable annotations are already separated into frontend overlays; the replacement page does not import duplicate annotations.

## Validation

Listing candidates uses a shared PDFium text-page handle and a single page-content preflight; it does not edit, save, and reopen every candidate. Applying an edit performs the stronger checks once for the selected run:

1. Reject unsupported source catalog features, confirm the selected object's text still matches the expected value, and require the unedited page copy to render identically to the original.
2. Apply a no-op through the selected native or verified stream-preserving path on a private copy, save, and reopen it. Compare all text, font/style/matrix/bounds, global glyph positions, and the page raster to detect discarded character positioning or changed appearance.
3. Apply the replacement on another private copy, save, and reopen it. Confirm exact replacement extraction, unchanged style/matrix, the original font or explicitly chosen font program, and unchanged surrounding text objects.
4. Check fit, crop bounds, and new text overlap. Compare page pixels outside the union of the old and new run bounds, with a three-pixel antialiasing allowance at the validation raster size.
5. Publish the new immutable source only after these checks succeed.

Native integration coverage lives in `src-tauri/tests/text_edit.rs`: all twelve font faces and four cropped page rotations; colored text and unchanged surrounding pixels; shorter, equal and narrower-but-longer replacements; rejection cases; source and file protection; chained edits; multipage isolation; annotation preservation; and a 400-run listing fixture. Run it with `cargo test --test text_edit` from `src-tauri` with the bundled PDFium library present.

Embedded-font integration coverage in `src-tauri/tests/embedded_text.rs` adds subset encodings, missing glyphs, explicit substitution and portable source bytes. Synthetic fixtures and their provenance are under `tests/fixtures/embedded-text/`.

Added text boxes have their own Font picker with all twelve standard Latin faces and supported installed/imported fonts. Font choice is retained through undo/redo, recovery, editable export/reopen and flattened export. Older Folio annotations default to Helvetica. See the [broader font support roadmap](font-support-roadmap.md) for installed, embedded and complex-font work.
