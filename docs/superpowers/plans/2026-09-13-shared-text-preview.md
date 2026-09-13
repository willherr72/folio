# Shared native text preview

Continue the approved #14 preview/export integration. This checkpoint extends
the existing semantic writer and SVG presentation patterns; it does not enable
complex-script text entry in the desktop editor.

1. Add a prepared semantic-text result containing an immutable PDF/layout and a
   versioned, serialize-only vector preview. Generate both from the same validated
   glyph-outline callbacks and positioned layout. Preserve current refusals and
   bound preview output. Existing PDF-only callers retain their API.
2. Add a small React vector-preview component. Use native outline definitions
   and placements, explicit PDF-to-display transforms and native color; do not
   ask browser fonts to shape the text. Keep definitions isolated between previews.
3. Extend the native fixture generator with preview JSON and PDFium rasters at
   150% and 300%. Compare actual browser SVG appearance against those rasters for
   ligatures, accents, marks, supplementary, Arabic and Indic cases at all source
   quarter turns. Preserve Unicode/source-font identity and save/reopen checks.
4. Run focused failing tests before implementation, native regressions, frontend
   tests/build, independent review and image inspection. Record measured limits
   and commit the checkpoint. Editing/IME, production wiring, undo/recovery and
   ownership of asynchronous prepared results remain the following integration.

Completed all four checkpoint steps. Results and remaining integration: [shared-text-preview.md](../../shared-text-preview.md).
