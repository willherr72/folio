# Existing PDF text editing

Folio edits existing text objects in supported PDFs. This changes the PDF content itself: exported and recovered pages contain the replacement as searchable text, and reopening an export exposes the edited run normally.

Folio supports visible, filled, top-level single-line text using the twelve Latin standard PDF fonts, plus positively verified embedded TrueType fonts. Current source builds also support representable accented Latin text in nominal Standard, WinAnsi and MacRoman standard-font resources. Each selected source operation must prove its exact encoding and advances; a familiar font name alone is insufficient. Embedded simple fonts and Type0 Identity-H CIDFontType2 fonts can reuse supported Latin characters, including precomposed accents, only when their code, glyph and width mappings are unambiguous. Replacement text stays on one line. The desktop replacement field accepts at most 1,000 characters; native and CLI/MCP requests are bounded to 4096 UTF-8 bytes. Original font, size, color and baseline are retained by default. Page crop offsets and intrinsic quarter-turn rotations are supported.

The standard-font extension covers eligible individual runs, groups, paragraphs
and reusable-form text, including saved re-editing. Overridden widths, descriptors,
custom mappings, ambiguous byte aliases and unavailable characters remain refused.
The accepted 0.23 portable package predates this extension; its standard-font path
still uses printable ASCII. The separate development candidate and its evidence
are tracked in the [quality record](product-quality-loop.md).

Development builds also accept explicitly WinAnsi-encoded, nonsymbolic embedded TrueType fonts without a `ToUnicode` table, as commonly produced by print-to-PDF drivers. The embedded Unicode glyph maps must agree and the PDF widths must match the font metrics. Missing maps in other encodings, conflicting maps, symbolic fonts, incorrect widths and malformed present `ToUnicode` tables still fail verification. This does not add characters missing from a subset font.

Current development source also recognizes equivalent encoding dictionaries with
an explicit WinAnsi or MacRoman base. Empty differences and exact, redundant ASCII
glyph assignments no longer block text or paragraph edits. Embedded simple fonts
still require WinAnsi and their usual Unicode/glyph/width proof. Actual remapping,
alternate glyph names, implicit bases and non-ASCII differences remain unsupported;
the usable repertoire still comes from the proved named base encoding. Source
encoding objects and all appearance/neighbor checks are retained.

For an eligible run whose subset lacks a needed character, choose **Choose substitute font…**, preview an installed or imported font, choose **Apply font**, then **Apply changes**. This explicitly changes the font while retaining size, color and baseline. The substitute travels inside the changed source PDF, so export/reopen and recovery do not depend on an installed font or separate imported file. The dialog keeps your typed replacement when a glyph or layout check fails.

Since v0.7.1, replacements can extend beyond the original run into available space while remaining within the visible page. Folio rejects new or expanded overlap with neighboring source text and graphics using conservative object bounds. Only a proven opaque, unstroked rectangular page fill painted behind the run is treated as a background. Images, forms and compound paths remain subject to conservative collision bounds, even when they cover the whole page. It does not reflow a paragraph or shrink the font to make an edit fit. Collision checks cover source page objects; separately placed annotations and overlays still need visual checking when expanding text.

Unverified or ambiguous font mappings, embedded CFF/Type1/Type3 programs, symbolic fonts, multi-character ligatures, and invisible/stroked/clipping text are unsupported. Scans do not produce editable source-text runs. Nested forms have the occurrence-editing support described below. Documents containing optional content (PDF layers) remain unsupported by these editing routes.

Current development builds preserve verified rectangular clipping, page transparency groups and marked-content ownership for eligible text, including tagged Word PDFs. Eligible single runs can also use an explicit substitute font within that preserved context. This does not enable arbitrary clipping shapes, transparency effects, custom encodings, pattern paints or ambiguous tag ownership. Some remaining encoding, text-state or placement problems can only be identified when applying the replacement; the editor reports the error and keeps the existing page and draft. See the [single-run font workflow](automation.md#explicit-fonts-for-single-text-runs) for the equivalent CLI/MCP controls.

## Styled paragraphs (current development build)

In current development builds, choose **Edit paragraphs**, then click outlined
text to open the paragraph workspace directly. **Edit text** keeps the individual
replacement workflow, including its **Edit paragraph…** action. Apply or Cancel
returns keyboard focus to the tool that opened the workspace. Folio suggests
adjacent pieces and supported wrapped lines, including mixed fonts, sizes, colors
and verified source highlights. Review the selected text and piece count. Use
**Adjust selection** and **Check selection** when the suggestion needs correction.
Column gaps, list boundaries, shifted baselines and ambiguous layouts can still
require manual selection. A complete paragraph previously saved by this editor can be restored
from any of its verified pieces, including its entered line breaks and formatting.

Current source builds also keep the replacement you already typed when choosing
**Edit paragraph…** from Edit text. A chosen substitute font follows that source
piece, while surrounding text keeps its formatting. The transfer is one local
paragraph Undo/Redo action. Use **Fit area to text** and **Preview paragraph**
before applying longer text. If preparation fails, the original text dialog
keeps your replacement and font for correction or copying.

Select words in **Paragraph text** to change their size, color or substitute
font. With only a caret, formatting applies to text typed next. Unselected styles
are preserved. **Undo** and **Redo** in the dialog restore draft edits; after
**Apply paragraph**, workspace undo restores the whole previous page version.
Plain-text paste keeps the surrounding styles. **Preview paragraph** renders
the actual PDF; source font labels and draft size/color cues do not promise an
exact embedded-font rendering inside the text input.

Missing characters in a PDF subset still require an explicit substitute font.
An imported font travels inside the saved PDF. Preview failures retain the
draft. Editing is bounded to 4096 UTF-8 bytes and 64 styles; changed manual
selections support at most 64 pieces. Larger saved selections require verified
native paragraph memory. Unverified graphics and neighboring content can still
cause conservative overlap refusals. General complex-script shaping
and cross-page paragraph flow are not supported.

Supported source highlights now follow paragraph edits. Folio must verify an
opaque rectangular mark painted behind one line and include all of its covered
text in the selection. The text input shows the verified color; the PDF preview
shows the actual measured highlight geometry. Wrapped lines get separate marks,
blank lines get none, and deleting the last highlighted word removes its mark.
Font, size and color changes retain the original highlight association. Saved
paragraphs restore it only after checking the PDF's paint and replayed appearance.
Arbitrary highlight-color changes and partially owned or complex source marks
remain unsupported in this paragraph workflow.

The highlight Windows checkpoint covers typing and draft history, native preview,
apply/undo/redo, save/reopen and re-edit at all four page rotations. An independent
reader checks eight saved outputs for unchanged neighboring text and identical
saved/re-edited pixels. This covers authored rectangular highlights; it does not
establish faithful editing of every foreign PDF or every annotation type.

The Windows checkpoint verifies insertion/deletion, selected size/color/font,
caret formatting, preview/apply, save/reopen/re-edit, native comments and all
four saved-page rotations. OS keyboard-layout/dead-key IME acceptance is still
open; composition testing used WebView2's automation input. Current source fixes
the reviewed **Reset position** issue: clearing Left or Top and then resetting
restores the coordinate even when the model position was already original.
Draft text and unrelated invalid fields are preserved. A subsequent packaged
Windows check verifies this fix and its undo behavior. The earlier styled-editor
checkpoint executable predates the fix; in that build, re-enter a valid coordinate
if reset leaves the field empty. Current source also measures initial paragraph
areas with the actual reflow fonts. When the original area is too small, it looks
for a compact extension within the page and clear of neighboring objects. Saved
verified paragraph areas remain unchanged. Obstructed or unsupported selections
can still require manual width/height adjustments or explicit font substitution.

## Earlier uniform paragraph builds

Choose **Edit text**, click a piece of the paragraph, then **Edit paragraph…**.
Earlier uniform builds suggest a paragraph around the clicked text and fill in its
text and area. Check the selected text and the outlined area. **Adjust selection**
shows individual pieces when the suggestion needs correction. The text editor is
shown first after a successful suggestion. **Cancel adjustment** returns to your
draft, area and preview. Checking the same selection preserves them too; checking
a different selection replaces them only after that selection passes validation.
Preview and Apply are unavailable while adjusting the pieces. Centered rows,
wrapped lines and split words can be suggested; list items, column gaps and larger
paragraph spacing act as boundaries. Nearby superscripts or uncertain mixed
formatting require manual selection. These suggestions remain subject to the same
native font, layout and appearance checks as an explicit selection. Portable
0.19.0-dev.2 predates suggestions and uses the manual workflow below.

Select every piece of that paragraph and choose **Check selection**. Review the
combined text, edit it, and adjust the area's width and height as needed. Choose
**Preview paragraph**, inspect the rendered PDF, then **Apply paragraph**.
Changing the text or area, or accepting a different selection, invalidates the preview. Overflow preserves
the draft; Cancel leaves the page unchanged. Apply is one undo step.

Current source builds also support **Left (pt)** and **Top (pt)**, or dragging the
paragraph area in **Original** view. Positions use the source page as shown in the
preview, including rotation saved in the PDF but excluding additional workspace
rotation. The entire area must stay inside the page.
**Reset position** restores the inspected position while keeping your edited text
and area size. Escape during a drag cancels that gesture; a cancelled or unchanged
drag retains the existing preview. A committed move requires a new preview before
Apply. These positioning controls are included in portable0.16.0-dev.1; the preserved
0.15.0-dev.1 checkpoint predates them.

Checkpoint0.21.0-dev.1 adds **Font size (pt)**, **Paragraph alignment** and
**Line spacing (× font size)**. Font size accepts1–300pt; alignment offers Left,
Center and Right; spacing accepts0.6–3 times the font size. Size changes scale
baseline spacing proportionally. Reset controls restore the inspected size or
spacing without discarding your text. Formatting changes require a new preview;
overflow retains the draft and leaves the source unchanged.

Alignment and regular spacing are estimated from selected line geometry. Unknown
alignment defaults to Left; unknown spacing defaults to1.2. The spacing control
rounds its initial display, while an untouched or reset value preserves the exact
native estimate. Explicitly typing a value overrides that estimate. Widely spaced
lines may need **Adjust selection**. Blank lines consume spacing, but source hard
breaks and blank-line semantics cannot be recovered reliably from PDF geometry
alone. Newer source builds retain verified settings for paragraphs edited in Folio,
as described below.

Source builds after portable0.21 also offer **Choose substitute font…** for a
uniform paragraph. Use an installed or imported TrueType font when the original
embedded subset lacks a character. **Apply font** selects it for the draft;
**Preview paragraph** shows the resulting PDF before **Apply paragraph** changes
the page. Canceling the picker keeps your draft and existing preview. **Use
original font** clears the choice while retaining text and other formatting.
The chosen font is embedded in the PDF, so subsequent edits and saved copies do
not need the imported file. Printable Latin paragraph limits still apply.

**Text color** changes the whole selected uniform paragraph. The picker starts
with its current color; **Use original color** restores the source value. Changing
or resetting color requires a new preview before applying. Color composes with
font substitution, size, alignment and spacing, and survives undo/redo and
save/reopen. Unchanged color is omitted from the edit request. Only opaque RGB
colors are supported; mixed-format selection limits remain unchanged.

Newer source builds also offer **Justify**. Soft-wrapped lines spread their interior
word spaces evenly to fill the area. Final lines, lines ending at an entered newline,
blank lines and single-word lines keep natural spacing. Preview before applying,
especially in narrow areas where gaps can become large. Size, spacing and explicit
font choices work with justification. Reopening estimates Justify only when multiple
compatible rows have uniformly expanded word gaps; ambiguous geometry defaults to
Left. Original semantic line breaks and edge whitespace in foreign PDFs remain
unrecoverable without additional information.

The faithful re-edit source checkpoint preserves entered hard breaks, blank lines
(including spaces on those lines), leading/trailing spaces, the chosen area,
alignment and spacing for complete supported Folio paragraph edits. Save/reopen,
page rotation, duplication and reordering retain these settings. Clicking a visible
line after a blank line can select the complete remembered paragraph. Partial manual
selections continue to use ordinary geometric inference.

The PDF carries bounded hints checked against its current text operators, font,
rendering state, glyph positions and reproduced layout. The hints contain break
counts and geometry, not a second copy of paragraph words. Stale, malformed or
overlapping hints are ignored. Other editors may remove them, and older Folio output
has none. Limits are 64 rows per remembered paragraph, 64 records per page and
512 KiB of hints per page; reaching capacity can discard older hints while retaining
visible PDF content. This source/debug feature is newer than portable0.21.

This first version supports Latin paragraphs that are horizontal in the PDF's
content coordinates and share one verified font resource, effective size, and color.
Current source builds also support saved page rotations of 90, 180 and 270 degrees;
the paragraph keeps its original direction. On a sideways page, the displayed area
height controls line length and its width controls space for additional lines.
Independent text rotation, shear and nonuniform scaling remain unsupported for
paragraphs. Saved-page rotation support is included in portable0.16.0-dev.1. It retains
the original font unless explicitly substituted and the original color unless changed, retains the original size unless explicitly changed,
wraps words with measured advance and ink bounds,
and treats entered newlines as hard breaks. Without a verified complete paragraph
hint, source line breaks become spaces when the selection is combined.
Paragraphs do not push other content or flow
across pages. Missing glyphs in the selected font, mixed formatting, selected highlighting,
marked content, and unsafe graphics/layout combinations are refused. Pages with added Folio text, ink, highlights and comments are supported. Native
Original/Edited previews include those overlays through the same appearance pipeline
as saving. The dialog displays source axes; workspace rotation and all overlays
remain intact on Apply. Composition failures retain the draft without enabling Apply.

Selected source strings are replaced with cursor-only advances; new searchable
lines use the original or explicitly substituted font resource. Before a preview is published, Folio checks
reopened text and formatting, actual unselected character positions, collisions,
and pixels outside the changed area. Cursor-only operators remain compatible
with subsequent paragraph, single-run, and eligible group edits. External readers
may insert line breaks and use a different page-wide extraction order after
reflow; exact external whitespace and reading order are not guaranteed. This is
content editing, not a secure-redaction tool.

Native saved notes retain their original rectangles, icon appearances and popup
ownership through paragraph, single-run and grouped text edits, including pages
rendered before editing and pages with saved rotation. Form-occurrence text editing
still refuses annotated sources; this does not add support for that case.

The supplied quiz's first question was tested with an inserted word: Calibri
remained 12 pt, two independent readers extracted the replacement, and a 2x
raster comparison found no changed pixels outside its paragraph area. The
private quiz remains outside the repository fixtures.

## Positioned text (v0.11.0)

This section records the original v0.11 behavior and its verification. Current
development builds additionally support explicit font changes for eligible
positioned text within verified clipping/tag contexts, as described above.

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

Development builds preserve bounded printer graphics settings through this same
stream path: disabled overprint (`OP`/`op` false), overprint mode 0 or 1, disabled
stroke adjustment (`SA` false), and finite smoothness (`SM`) from 0 to 1. The
settings and other operators remain intact. Font substitution on these pages
remains unavailable; transparency and other unsupported graphics state still
refuse editing. These compatibility improvements are not part of the published
v0.13.0 installer.

Passing edit-eligibility checks does not guarantee that a longer replacement will
fit. In a split paragraph, inserting a word can overlap the next separately
positioned piece. That overlap can also change the reader's extracted text; Folio
now reports the collision before that secondary mismatch and explains that the
paragraph does not reflow. Exact replacement text is still required for every
successful edit. The original page and replacement draft remain available after
a refusal. The separate paragraph editor below can reflow a uniform-format selection.

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
A group cannot choose a substitute font. Isolated adjacent pieces with fractional
font widths can be edited together when their character positions and surrounding
content are preserved. Selections that overlap other content may still be refused.
Compatible rotated or transformed pieces can pass the same preservation proof.
On rotated pages, groups can include verified accented Latin and single-character
pieces. Selected pieces must advance left to right along one source baseline;
unselected text with a different orientation does not by itself prevent grouping.
The rotated selection check remains bounded to 64 page objects and 4096 characters.
Unchanged replacements preserve the original separate pieces and exact appearance.
Longer replacements can use available space,
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

Current source builds support proved nominal Type1 Latin fonts with their supported Standard, WinAnsi or MacRoman characters,
and verified embedded TrueType fonts with supported Latin characters, including
precomposed accents. Embedded simple fonts and Type0 Identity-H fonts use the same
code/glyph/width verification as top-level editing. Readable font labels and subset
guidance appear in the dialog. A missing glyph retains the draft. Newer source
builds offer **Choose substitute font…** for these supported occurrences: preview
an installed or imported font, choose **Apply font**, then **Apply changes**.
The chosen font changes only this placement, retaining its size, color and baseline.
The complete font is embedded so saved copies can be reopened and edited without
the imported file. Choosing a different font with unchanged text is supported.
**Use original font** resets the choice without discarding the replacement draft.
An edit that does not fit keeps both the draft and chosen font available for retry.

Newer source builds preserve bounded rotation, shear, reflection and scaling in
the selected text and its enclosing forms. The edit retains the original angle
and transform, including when choosing a substitute font. These are compatibility
improvements; the replacement dialog does not add rotation or skew controls.

Text must be nonblank and use the conservative text-only content grammar.
Ordinary surrounding spaces are retained from the verified source encoding.
Unverified/custom font mappings,
grouping and paragraph editing remain unsupported inside forms. Text must fit
inside the page and every ancestor form box without
new overlap. Tagged/layered sources, unsupported annotation/appearance graphs,
clipping, transparency, images and uncertain inherited state remain refusal cases.
The complete-document desktop path preserves supported shared interactive fields,
notes and links; legacy isolated-page adapters still refuse source annotations.
Existing top-level text, added annotations and their font features retain their
previous behavior.

Current source also preserves explicit character spacing, word spacing, horizontal
text scale, text rise and leading inside these forms, including inherited and
saved graphics state. These values are retained from the PDF; the replacement
dialog does not add controls to change them. Filled text remains required. Each
glyph-bearing show needs a verified active font and a known initial line position.
For consecutive pieces, an edit preserves the following piece's original cursor
with a bounded spacing adjustment. Unsafe or ambiguous geometry still fails
verification, and longer replacements still need available room.

When a single-byte source font uses nonzero word spacing, the dialog says
**Keep the original font to preserve this text’s word spacing.** Original-font
edits remain available. Installing a different font would change how that spacing
is applied, so Folio also rejects such a request natively. Two-byte Identity-H
fonts do not apply PDF word spacing to their two-byte space code and can still
use substitution. Reusing the exact verified original font program and encoding
preserves its source spacing even when the subset has no matching Unicode map.

The 2026-10-24 development checkpoint verifies these edits through native
regressions, independently authored reference PDFs, and actual Windows
edit/undo/redo/save/reopen workflows. It includes rotated, reflected and sheared
placements, decimal page scaling, recovery without the original file, and
preservation of untouched shared copies. Some valid layouts still fail the
conservative original/private-validation appearance comparison. The checkpoint
does not establish support for every PDF producer, font or text-show instruction.

Supported nested forms also accept positioned `TJ` arrays: text split into encoded
segments with explicit numeric spacing adjustments. The numbers stay at their
original logical character slots, including when an explicit substitute font uses
a different byte width. Unchanged text preserves all source adjustments. Shorter
replacements discard adjustments at or beyond their new end; longer replacements
fill the old slots and append after them. Retyping text after shortening does not
recover discarded adjustments; Undo restores the complete earlier source.

Numeric-only cursor arrays are retained without becoming selectable text objects.
Multiple glyph-bearing shows may share a text object after `Tm`, `Td`, `TD`, `T*`
or a quote operator establishes the line position. Consecutive `Tj`/`TJ` pieces
can be edited separately while retaining later positions. Balanced empty text objects
can establish a font for later lines, as ordinary ReportLab output does. Fonts
retain their resolved resource across text objects and saved graphics scopes;
substitution selects and restores a font around only the chosen show. A child
form's reused font name cannot change the meaning of the font selected earlier.
Double quotes retain their word/character spacing, and `TD` retains its leading
change. Split encoded characters, malformed arrays, whitespace-only pieces and
unverified text mappings remain unsupported. This compatibility work preserves the
existing font, clipping, collision and surrounding-pixel checks. Native and
Windows save/reopen checks cover standard, simple embedded and Identity-H fonts,
including transformed/scaled placements and explicit font substitution; they do
not establish compatibility with every PDF producer.

The development renderer now preserves inherited leading, text rise and horizontal
scale in nested forms. The prior PDFium binary lost those values and caused valid
text to fail Folio's appearance comparison. A minimal pinned source correction
fixes the renderer while keeping that guard intact. Standard, embedded simple and
Identity-H cases pass real editing/export/reopen checks against authored references;
three representative cases also pass the actual Windows UI workflow and independent
pixel/glyph checks. Other unsupported layouts still remain guarded. See
[the diagnosis, correction and exact acceptance limits](pdfium-form-inheritance.md).

The renderer also retains fractional horizontal widths in supported simple and
CID fonts. This prevents a small downstream shift when replacing a piece with a
different length. The pinned build includes both corrections; its direct probes
exercise explicit widths, MissingWidth, CID ranges and DW, plus integer controls.
Consecutive editing tests cover shared copies, explicit spacing, transformed forms,
font substitution, surrounding ordinary spaces and bounded repeated corrections.
Independent export checks evaluate actual PDF cursor math and unchanged pixels
outside the edit. MuPDF rounds some fractional widths, so its coordinate check
allows 0.01 pt while native integration uses 0.001 pt; the observed discrepancy
in the current synthetic corpus is below 0.002 pt. This is a compatibility boundary,
not a claim of universal renderer agreement.

An inherited leaf font resource can be preserved. An intermediate form without
its own resource dictionary is refused: adding a dictionary there can change
lookup semantics. The exact resource chain and invocation path identify the
selection, even when several copies have identical text and font names.

A bounded graph check runs for the form-editing operations before native page
copy/traversal. It refuses cycles, excessive depth/expanded work and uncertain
inline-image parsing. Direct resource copies have cumulative byte/value limits
checked before allocation. It does not change ordinary PDF opening or claim to harden
all native rendering. A private validation page is flattened only for comparing
font/text geometry; published pages keep their nested forms. Actual no-op cloning
must preserve the full page text/geometry and raster. Changed output is reopened
and checked before the new immutable source becomes available. Rotated text-bound
corners are checked in each ancestor box's own coordinate system, so a skewed or
rotated box does not silently become a larger rectangular clipping area. Selection
hit targets and neighbor-collision checks still use conservative page-axis bounds.

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

The October 2026 print-to-PDF compatibility check reproduced a one-page source
whose 50 nonempty text pieces were refused. All 50 now pass listing checks.
Separate date and heading edits save and reopen through the native engine;
MuPDF and pypdf extract exactly the expected changes, and 2x raster comparisons
find zero changed pixels outside each edited region. The supplied document stays
outside tracked test fixtures. Regressions use the existing redistributable fonts
and generated PDFs to cover missing `ToUnicode`, conflicting glyph maps, invalid
font flags/widths, malformed mappings, printer-state preservation and substitution
refusal.

For local diagnostics, `cargo run --manifest-path src-tauri/Cargo.toml --example
inspect-existing-text -- <PDF>` lists native edit eligibility. Optional arguments
`<object-index> <replacement> <new-output.pdf>` exercise a page-one edit and
reopen it. The output must be a new file; this probe never overwrites the input.

Added text boxes have their own Font picker with all twelve standard Latin faces and supported installed/imported fonts. Font choice is retained through undo/redo, recovery, editable export/reopen and flattened export. Older Folio annotations default to Helvetica. See the [broader font support roadmap](font-support-roadmap.md) for installed, embedded and complex-font work.

Existing-text, group and paragraph dialogs prefer a readable full name from an
already verified embedded TrueType program. This works after save/reopen and after
the font picker releases its registry entry. The optional `fontDisplayName` is
informational; raw `fontName`, resource identities, glyph mappings, font programs
and edit eligibility are unchanged. Standard fonts and programs without a usable
full name retain the existing raw-name fallback. Verified embedded form occurrences
also expose this label; it does not relax mapping or layout checks.

Name selection shares the picker path, prefers an English record and otherwise
uses a decodable full-name record. It bounds record scanning and decoded length,
rejects blank/control/directional labels, and trims outer whitespace. An unusable
name does not make an otherwise supported font ineligible for editing.


### Paragraphs on pages with shared form fields

The current development build can edit an eligible paragraph on a page whose form
fields are shared with other pages. Preview shows the selected page, including a
selected duplicate. Applying the paragraph preserves the connected field, internal
links, bookmarks, comments, crop and workspace rotation. Undo restores the previous
paragraph and page connections in one step. Saving a copy and reopening it keeps
supported paragraph formatting and highlights available for later edits.

When added words do not fit, use **Fit area to text**, inspect **Preview paragraph**,
and then **Apply paragraph**. Existing collision, font and supported-text limits
still apply. Paragraphs do not push neighboring content onto another page.

### Grouped text on pages with shared form fields

**Edit together…** now supports eligible same-line pieces on pages with shared
form fields. Select the nearby pieces, choose **Check selection**, and apply the
replacement. Editing a duplicate changes only that selected copy while preserving
linked fields, internal links, bookmarks and comments. Undo/Redo, saved copies and
workspace recovery retain these connections.

The original font and placement rules still apply; text must fit without changing
surrounding content. Use paragraph editing for supported reflow.

### Reusable text on pages with shared form fields

**Edit text** also supports eligible text inside reusable PDF Form XObjects on
documents with shared interactive fields. Editing one occurrence, including on a
duplicated page, preserves the other occurrences and the connected document's
fields, notes, internal links and bookmarks. Crop and workspace rotation remain.

Choose **Choose substitute font…** when the original embedded font lacks needed
characters. The chosen font is embedded in the saved PDF and remains available
when reopening it. Undo/Redo and workspace recovery retain these edits and field
connections. Leaving both text and font unchanged keeps Apply disabled; cancel
the dialog to return to the document.

Eligible reusable text also supports **Edit paragraph…** in current development
builds. Review the suggested membership and preview before applying: the grouping
scope is the selected outer Form occurrence, which may contain several inner
objects. The edited paragraph becomes ordinary page content for subsequent edits;
other uses of the shared Form stay unchanged. CLI/MCP exposes the same inspected
membership through [nested text and paragraph automation](automation.md).
Grouping arbitrary nested pieces, unsupported scripts and unverified producers
remain compatibility gaps.

### Spaces in existing text

For eligible ordinary text, the editor now uses the text stored in the PDF's
content instructions. Spaces added by a PDF reader to separate nearby objects
no longer become draft characters or prevent an otherwise valid edit. Actual
leading, trailing and repeated spaces remain part of the text.

Selection checks, font and position validation, overlap protection and unchanged
neighbor checks still apply. Verified embedded subsets are also eligible when a
space added by the reader is absent from the font's mapping: the original source
text is checked separately. A real new character missing from the subset still
requires **Choose substitute font…**. That chosen font is embedded in saved copies,
including when the new character is a space. Uniform and styled paragraph editing
also check actual source characters when determining font eligibility, including
saved paragraph replay. A moved paragraph may change a reader-added separator on
nearby text; accepting that difference requires matching decoded neighbor text,
glyphs, style, position and surrounding pixels. Group editing retains its existing
eligibility rules. Unsupported scripts and unverified encodings remain gaps.
