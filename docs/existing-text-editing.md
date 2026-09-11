# Existing PDF text editing

Folio edits existing text objects in supported PDFs. This changes the PDF content itself: exported and recovered pages contain the replacement as searchable text, and reopening an export exposes the edited run normally.

Folio supports visible, filled, horizontal, top-level text using the twelve Latin standard PDF fonts, plus positively verified embedded TrueType fonts. The standard-font path uses printable ASCII. Embedded simple fonts and Type0 Identity-H CIDFontType2 fonts can reuse supported Latin characters, including precomposed accents, only when their code, glyph and width mappings are unambiguous. Replacement text stays on one line with at most 1,000 characters. Original font, size, color and baseline are retained by default. Page crop offsets and intrinsic quarter-turn rotations are supported.

For an eligible run whose subset lacks a needed character, choose **Choose substitute font…**, preview an installed or imported font, choose **Apply font**, then **Apply changes**. This explicitly changes the font while retaining size, color and baseline. The substitute travels inside the changed source PDF, so export/reopen and recovery do not depend on an installed font or separate imported file. The dialog keeps your typed replacement when a glyph or layout check fails.

Since v0.7.1, replacements can extend beyond the original run into available space while remaining within the visible page. Folio rejects new or expanded overlap with neighboring source text and graphics using conservative object bounds. Only a proven opaque, unstroked rectangular page fill painted behind the run is treated as a background. Images, forms and compound paths remain subject to conservative collision bounds, even when they cover the whole page. It does not reflow a paragraph or shrink the font to make an edit fit. Collision checks cover source page objects; separately placed annotations and overlays still need visual checking when expanding text.

Unverified or ambiguous font mappings, embedded CFF/Type1/Type3 programs, symbolic fonts, multi-character ligatures, invisible/stroked/clipping text, and rotated/scaled/skewed text objects are unsupported. Scans and text nested inside form objects do not produce editable runs. Documents containing optional content (PDF layers) are disabled, because copying a page can lose the catalog's visibility settings. Pages containing clipping paths, unverified custom font encodings, explicit TJ character adjustments, named pattern paints, or advanced graphics state are conservatively disabled as a whole. Explicit substitution additionally refuses marked-content/tagged text. Some remaining encoding or text-state problems can only be identified when applying the replacement; the editor reports the error and keeps the existing page.

## Preservation and history

The backend copies only the selected source page and edits that private copy. It never mutates a previously opened source. Applying the change creates a new one-page source; the frontend retains the workspace page identity, rotation, and annotation overlays while switching its source reference. Other pages and duplicate instances of the old source remain unchanged. Undo and redo select the retained immutable sources.

The original filesystem path is inherited through every edit, preserving the existing protection against overwriting an original PDF. The new source snapshot holds the edited PDF bytes, so printing, export, and recovery use the same changed content. Imported editable annotations are already separated into frontend overlays; the replacement page does not import duplicate annotations.

## Validation

Listing candidates uses a shared PDFium text-page handle and a single page-content preflight; it does not edit, save, and reopen every candidate. Applying an edit performs the stronger checks once for the selected run:

1. Reject unsupported source catalog features, confirm the selected object's text still matches the expected value, and require the unedited page copy to render identically to the original.
2. Set the same text on a private copy, save, and reopen it. Compare all text, font/style/matrix/bounds, global glyph positions, and the page raster to detect discarded character positioning or changed appearance.
3. Apply the replacement on another private copy, save, and reopen it. Confirm exact replacement extraction, unchanged style/matrix, the original font or explicitly chosen font program, and unchanged surrounding text objects.
4. Check fit, crop bounds, and new text overlap. Compare page pixels outside the union of the old and new run bounds, with a three-pixel antialiasing allowance at the validation raster size.
5. Publish the new immutable source only after these checks succeed.

Native integration coverage lives in `src-tauri/tests/text_edit.rs`: all twelve font faces and four cropped page rotations; colored text and unchanged surrounding pixels; shorter, equal and narrower-but-longer replacements; rejection cases; source and file protection; chained edits; multipage isolation; annotation preservation; and a 400-run listing fixture. Run it with `cargo test --test text_edit` from `src-tauri` with the bundled PDFium library present.

Embedded-font integration coverage in `src-tauri/tests/embedded_text.rs` adds subset encodings, missing glyphs, explicit substitution and portable source bytes. Synthetic fixtures and their provenance are under `tests/fixtures/embedded-text/`.

Added text boxes have their own Font picker with all twelve standard Latin faces and supported installed/imported fonts. Font choice is retained through undo/redo, recovery, editable export/reopen and flattened export. Older Folio annotations default to Helvetica. See the [broader font support roadmap](font-support-roadmap.md) for installed, embedded and complex-font work.
