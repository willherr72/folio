# Text-area layout checkpoint (#16)

## Scope
Build a native development API and reproducible preview/PDF reader fixture before enabling a desktop text-area tool. This is an architectural checkpoint under the authorized issue roadmap. It does not close #16, alter existing overlays, or widen #14 bidi support. No application IPC/schema/version change in this checkpoint.

## Contract
`layout_text_area(font: &FontAsset, request: &TextAreaRequest) -> EngineResult<TextAreaLayout>`.
Request fields: version=1, text, font_size, width, height, inset, line_spacing (multiplier), alignment (left/center/right), direction (LTR only), ligatures. Dimensions are finite PDF points, at most 14400; font size 1..512; inset nonnegative leaving positive inner area; spacing 1..3. Text <=4096 scalars/16384 UTF-8 bytes, lines <=256. Support Latin/Common/Inherited scripts with explicit LF and CRLF hard breaks; reject bare CR, controls, bidi controls, RTL and other scripts before line splitting. NBSP is nonbreaking; do not split graphemes or unbreakable words. Word boundaries are ASCII space runs only; this is not full Unicode line breaking.

Output retains exact logical text, font identity and request, with ordered lines containing source `TextRange`, delimiter `TextRange`, break kind (soft/hard/end), baseline x/y in top-left area coordinates, advance width and optional reshaped ShapedText (empty rows have none). Delimiters preserve every consumed whitespace/newline byte, but are not painted; reconstructing source from line+delimiter spans is exact. Returned glyph ranges are documented as line-local; line source range maps to global positions. Hard breaks preserve empty rows, including a final empty row. Automatic wraps insert no source characters.

Use native metrics and actual ink to place/check content. Alignment is explicit. Oversized unbreakable tokens produce horizontal overflow, never smaller font or character loss. All lines are retained within the line-count bound; vertical overflow is explicit. API includes `can_export`/overflow diagnostics. Generated PDF evidence refuses overflow; no hidden clipping. Cumulative shape work and emitted glyph counts are bounded before repetition can become quadratic. Budget exhaustion returns an error, not partial output. Unsupported drafts remain caller-owned.

## Verification and integration gate
Deterministic serde request roundtrips recompute identical layout. Test accents/ligatures, spaces/NBSP, newlines/blank lines, narrow/long tokens, alignment/height/font changes, malformed inputs, exact source ranges and bounded work. Reference each emitted line against a fresh independent shape of that final line. Reproducible native example exports ordinary existing shaped overlays at chosen baselines, writes native preview/layout manifests, reads PDFium Unicode/geometry and resaves. Python compares MuPDF/pypdf and records exact logical text and per-line results separately. Do not normalize inserted newlines into a false exact-copy pass. Preview compares actual vector paths with PDF raster. If external soft-wrap copy does not retain exact logical text, preserve the failure and leave production area serialization/selection gated.

## Alternatives
Browser wrapping would diverge from native metrics. Splitting existing paragraphs would alter source content. Both are excluded. The selected native-layout checkpoint provides concrete evidence before adding a persisted area schema, canvas resize, search/selection, undo and recovery integration.

Ruling: trailing ASCII spaces join each line delimiter, including before hard newlines or end-of-text; their exact bytes remain source-owned. Non-ASCII whitespace-only rows (including NBSP-only rows) are explicitly refused because existing shaping requires visible content. Do not replace shaping with nominal metrics.

Checkpoint limitation: a non-ASCII whitespace-only token encountered during candidate shaping is also refused, even when another token would add visible ink. Existing shaper geometry/font refusals are inherited, including its large-advance limit; valid request dimensions do not guarantee the shaper accepts every candidate. These paths return errors without partial layouts.
