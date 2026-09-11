# Broader font support

Folio currently offers twelve standard Latin PDF faces for added text: Helvetica, Times and Courier, each regular, bold, italic and bold italic. Existing-text edits retain the original verified font by default, with explicit substitution available for eligible runs.

## Installed and imported fonts — available in v0.8.0

Select a text box, click **More fonts…** under Font, then search installed faces or choose **Import font…** for a local file. Select a face to preview your text, then choose **Apply font**. Each installed style is a separate face. The twelve standard PDF faces remain available directly in the Font dropdown.

Folio accepts static TrueType-outline TTF and compatible OTF files with installable or editable embedding permissions. It rejects CFF/CFF2 outlines, variable/color fonts, collections (TTC/OTC), malformed files and restricted or preview/print-only fonts with an explanation. Full font embedding preserves no-subsetting permissions. Fonts are read locally; nothing is uploaded or installed into Windows.

Covered BMP Latin, extended Latin, Greek, Cyrillic and selected punctuation/symbols are supported. Newlines create explicit lines. Combining sequences, bidirectional/shaping scripts, CJK and emoji remain outside this first stage, even when the font contains those glyphs. Prefer precomposed accents such as é. Missing or unsupported characters remain in Content with an error so you can correct them; PDF export refuses them rather than substituting another font.

The native registry retains immutable font bytes by SHA-256. WebView FontFace preview and PDF Type0/CIDFontType2 embedding use those same bytes, with Unicode mappings for copying/search. Custom fonts travel inside editable and flattened PDFs; reopening needs neither an installed font nor the imported file. Undo/redo and open tabs share resources; recovery keeps exact font sidecars. The registry permits 64 distinct fonts, 16 MiB per font and 128 MiB total. Closing the last owning tab releases its resources; undo history can continue to own a previously selected font until that history is removed.

The same chooser provides an explicit substitute for eligible existing text. Selecting a preview does not alter the PDF until you apply the text edit.

## Existing embedded and subset fonts — v0.9.0

Horizontal Latin runs with reliable Unicode mappings can reuse verified embedded TrueType programs. Folio inspects PDF character encoding, glyph IDs and widths, then proves the requested replacement can use the existing font. Supported simple subsets and Type0 Identity-H CIDFontType2 resources retain their original size, color and baseline. Save/reopen geometry and pixel checks protect the surrounding content.

A subset may omit needed letters. The editor identifies the missing character and keeps your draft. Choose **Choose substitute font…**, preview an installed or imported full font, choose **Apply font**, then **Apply changes**. The new source embeds that font and its Unicode mapping. There is no silent fallback. Missing or ambiguous mappings, multi-character ligatures, unsupported font programs and unsafe layouts remain rejected; a font chooser cannot reconstruct absent source meaning.

## Complex scripts and positioned text

Arabic, Indic scripts, combining marks and ligatures require shaping: converting text into glyphs and their positions. HarfBuzz handles shaping, while bidirectional ordering, line breaking and text layout need additional integration. Preserve mappings between Unicode text and shaped glyph clusters for editing, selection and copying. [HarfBuzz's description of shaping](https://harfbuzz.github.io/what-is-harfbuzz.html).

PDF layout complexity is a separate concern. A visible word can be divided among individually positioned objects, and a paragraph need not exist as a paragraph in the file. Support those cases incrementally: character spacing and transforms, groups of adjacent runs, nested form objects, then explicit text areas with line wrapping and reflow. Preserve shared-object ownership so changing one occurrence does not accidentally alter other pages.

The [complex-text investigation](complex-text-design.md) records shaping and reader experiments, including a combining-mark extraction incompatibility that remains unresolved. Implementation is tracked in [#14](https://github.com/willherr72/folio/issues/14) (shaping and Unicode), [#15](https://github.com/willherr72/folio/issues/15) (positioned runs and forms), and [#16](https://github.com/willherr72/folio/issues/16) (bounded wrapping). [#13](https://github.com/willherr72/folio/issues/13) remains the umbrella. Those scripts and layout features are not enabled by the investigation.
