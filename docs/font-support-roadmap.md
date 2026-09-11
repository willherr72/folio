# Broader font support

Folio currently offers twelve standard Latin PDF faces for added text: Helvetica, Times and Courier, each regular, bold, italic and bold italic. Existing-text edits retain the original supported font. Choosing a font for an added text box does not yet enable changing the font of an existing run.

## Installed and imported fonts

The next useful step is a document font registry. Load an installed or user-selected TrueType/OpenType font, check supported characters and embedding permissions, and retain its bytes with the document. Use those same bytes for the WebView preview and native export so appearance does not depend on a substitute installed on the reader's machine. Recovery must retain the font resource, and unused resources need bounded ownership and cleanup.

PDFium provides font-data extraction and font-loading APIs, including loading a Type 2 CID font with caller-provided Unicode and glyph mappings. We need to verify the exact APIs available in our bundled library and implement the registry, mapping and persistence around them. These APIs are useful building blocks; they do not supply a complete editing workflow. [PDFium public editing API](https://pdfium.googlesource.com/pdfium/+/refs/heads/main/public/fpdf_edit.h).

## Existing embedded and subset fonts

Start with horizontal Latin runs that already have reliable Unicode mappings. Inspect the embedded font program, PDF character encoding, glyph IDs and widths. Prove that each requested replacement character can be encoded and rendered with the existing font; preserve the selected run's transform and layout and retain the current save/reopen and pixel-preservation checks.

A subset may omit letters needed by the replacement. In that case, find a usable full font or let the user choose a substitute and preview its appearance. Update font resources and text-to-Unicode mappings together so rendered letters, search and copy agree. Do not silently replace missing letters or assume extracting a subset recovers the complete font. Test missing glyphs, misleading font names, missing/ambiguous mappings, ligatures, chained edits and recovery independently.

## Complex scripts and positioned text

Arabic, Indic scripts, combining marks and ligatures require shaping: converting text into glyphs and their positions. HarfBuzz handles shaping, while bidirectional ordering, line breaking and text layout need additional integration. Preserve mappings between Unicode text and shaped glyph clusters for editing, selection and copying. [HarfBuzz's description of shaping](https://harfbuzz.github.io/what-is-harfbuzz.html).

PDF layout complexity is a separate concern. A visible word can be divided among individually positioned objects, and a paragraph need not exist as a paragraph in the file. Support those cases incrementally: character spacing and transforms, groups of adjacent runs, nested form objects, then explicit text areas with line wrapping and reflow. Preserve shared-object ownership so changing one occurrence does not accidentally alter other pages.

Recommended order: installed/imported font resources for added text; simple embedded-font reuse with glyph checks; explicit substitution when a subset is insufficient; then complex shaping and layout. Each stage needs exported/reopened PDFs checked by independent readers, including selection, search, rotation, undo/redo and recovery. This roadmap does not claim these features are implemented yet.
