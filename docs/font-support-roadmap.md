# Broader font support

Folio currently offers twelve standard Latin PDF faces for added text: Helvetica, Times and Courier, each regular, bold, italic and bold italic. Existing-text edits retain the original supported font. Choosing a font for an added text box does not yet enable changing the font of an existing run.

## Installed and imported fonts — available in v0.8.0

Select a text box, click **More fonts…** under Font, then search installed faces or choose **Import font…** for a local file. Each installed style is a separate face. The twelve standard PDF faces remain available directly in the Font dropdown.

Folio accepts static TrueType-outline TTF and compatible OTF files with installable or editable embedding permissions. It rejects CFF/CFF2 outlines, variable/color fonts, collections (TTC/OTC), malformed files and restricted or preview/print-only fonts with an explanation. Full font embedding preserves no-subsetting permissions. Fonts are read locally; nothing is uploaded or installed into Windows.

Covered BMP Latin, extended Latin, Greek, Cyrillic and selected punctuation/symbols are supported. Newlines create explicit lines. Combining sequences, bidirectional/shaping scripts, CJK and emoji remain outside this first stage, even when the font contains those glyphs. Prefer precomposed accents such as é. Missing or unsupported characters remain in Content with an error so you can correct them; PDF export refuses them rather than substituting another font.

The native registry retains immutable font bytes by SHA-256. WebView FontFace preview and PDF Type0/CIDFontType2 embedding use those same bytes, with Unicode mappings for copying/search. Custom fonts travel inside editable and flattened PDFs; reopening needs neither an installed font nor the imported file. Undo/redo and open tabs share resources; recovery keeps exact font sidecars. The registry permits 64 distinct fonts, 16 MiB per font and 128 MiB total. Closing the last owning tab releases its resources; undo history can continue to own a previously selected font until that history is removed.

This picker changes added text boxes. Reusing fonts from existing PDF content is the next separate stage below.

## Existing embedded and subset fonts

Start with horizontal Latin runs that already have reliable Unicode mappings. Inspect the embedded font program, PDF character encoding, glyph IDs and widths. Prove that each requested replacement character can be encoded and rendered with the existing font; preserve the selected run's transform and layout and retain the current save/reopen and pixel-preservation checks.

A subset may omit letters needed by the replacement. In that case, find a usable full font or let the user choose a substitute and preview its appearance. Update font resources and text-to-Unicode mappings together so rendered letters, search and copy agree. Do not silently replace missing letters or assume extracting a subset recovers the complete font. Test missing glyphs, misleading font names, missing/ambiguous mappings, ligatures, chained edits and recovery independently.

## Complex scripts and positioned text

Arabic, Indic scripts, combining marks and ligatures require shaping: converting text into glyphs and their positions. HarfBuzz handles shaping, while bidirectional ordering, line breaking and text layout need additional integration. Preserve mappings between Unicode text and shaped glyph clusters for editing, selection and copying. [HarfBuzz's description of shaping](https://harfbuzz.github.io/what-is-harfbuzz.html).

PDF layout complexity is a separate concern. A visible word can be divided among individually positioned objects, and a paragraph need not exist as a paragraph in the file. Support those cases incrementally: character spacing and transforms, groups of adjacent runs, nested form objects, then explicit text areas with line wrapping and reflow. Preserve shared-object ownership so changing one occurrence does not accidentally alter other pages.

Next: simple embedded-font reuse with glyph checks (#12), explicit substitution when a subset is insufficient, then complex shaping and layout (#13). Each stage needs exported/reopened PDFs checked by independent readers, including selection, search, rotation, undo/redo and recovery. Only the installed/imported stage is implemented here.
