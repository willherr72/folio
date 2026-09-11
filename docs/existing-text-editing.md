# Existing PDF text editing

Folio edits existing text objects in supported PDFs. This changes the PDF content itself: exported and recovered pages contain the replacement as searchable text, and reopening an export exposes the edited run normally.

The first version supports visible, filled, horizontal, top-level text using the twelve Latin standard PDF fonts: Helvetica, Times, Courier, and their bold/italic variants. Replacement text must contain 1–1,000 printable ASCII characters in the editing dialog on one line. The existing font, size, color, and baseline are retained. Page crop offsets and intrinsic quarter-turn rotations are supported.

The replacement must fit before the original run's right edge and remain within the visible page. Longer character counts can fit when their glyphs are narrower. Folio rejects new overlap with surrounding text. It does not reflow a paragraph or shrink the font to make an edit fit.

Embedded, subset, custom and symbolic fonts, invisible/stroked/clipping text, and rotated/scaled/skewed text objects are unsupported. Scans and text nested inside form objects do not produce editable runs. Documents containing optional content (PDF layers) are disabled, because copying a page can lose the catalog's visibility settings. Pages containing clipping paths, custom font encodings, explicit TJ character adjustments, or advanced graphics state are conservatively disabled as a whole. Some remaining encoding or text-state problems can only be identified when applying the replacement; the editor reports the error and keeps the existing page.

## Preservation and history

The backend copies only the selected source page and edits that private copy. It never mutates a previously opened source. Applying the change creates a new one-page source; the frontend retains the workspace page identity, rotation, and annotation overlays while switching its source reference. Other pages and duplicate instances of the old source remain unchanged. Undo and redo select the retained immutable sources.

The original filesystem path is inherited through every edit, preserving the existing protection against overwriting an original PDF. The new source snapshot holds the edited PDF bytes, so printing, export, and recovery use the same changed content. Imported editable annotations are already separated into frontend overlays; the replacement page does not import duplicate annotations.

## Validation

Listing candidates uses a shared PDFium text-page handle and a single page-content preflight; it does not edit, save, and reopen every candidate. Applying an edit performs the stronger checks once for the selected run:

1. Reject unsupported source catalog features, confirm the selected object's text still matches the expected value, and require the unedited page copy to render identically to the original.
2. Set the same text on a private copy, save, and reopen it. Compare all text, font/style/matrix/bounds, global glyph positions, and the page raster to detect discarded character positioning or changed appearance.
3. Apply the replacement on another private copy, save, and reopen it. Confirm exact replacement extraction, unchanged font/style/matrix, and unchanged surrounding text objects.
4. Check fit, crop bounds, and new text overlap. Compare page pixels outside the union of the old and new run bounds, with a three-pixel antialiasing allowance at the validation raster size.
5. Publish the new immutable source only after these checks succeed.

Native integration coverage lives in `src-tauri/tests/text_edit.rs`: all twelve font faces and four cropped page rotations; colored text and unchanged surrounding pixels; shorter, equal and narrower-but-longer replacements; rejection cases; source and file protection; chained edits; multipage isolation; annotation preservation; and a 400-run listing fixture. Run it with `cargo test --test text_edit` from `src-tauri` with the bundled PDFium library present.
