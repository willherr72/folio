# Text-area persistence

Preserve original text, exact font bytes and versioned layout settings through editable PDF save/reopen and crash recovery. This follows the reader-compatible copy policy: Folio retains exact logical text; external readers may introduce whitespace differences.

Extend text overlays with optional `area` settings: version 1, width, height, inset, line spacing and left/center/right alignment. Existing text/font size/font ID/color/rotation and shaping fields remain authoritative. Areas require a custom font and explicit LTR shaping. Existing single-line overlays retain their wire format and behavior.

Prepare multiline appearances using the existing bounded Latin layout and native single-line preparation. Combine per-line semantic fonts with unique resource names, translate glyphs and character boxes into the saved rectangle, and retain original delimiters in Folio's character data. Bound aggregate preparation work and output. Refuse overflowing export; recovery must preserve unfinished overflowing or unsupported text without attempting layout.

Editable PDF records use version 4 for areas. Reopen verifies the exact embedded font, regenerated appearance, bounds and annotation fields before removing the native annotation and restoring an editable overlay. Unsupported or altered records remain native annotations. Old record versions cannot claim area support.

Frontend history snapshots and native preview cache keys include all area settings. Reopened areas use native previews and retain LTR/custom-font requirements when edited. This milestone does not add area creation or resizing controls.

Verify exact source text including NBSP, repeated spaces, CRLF and blank lines; legacy compatibility; stable save/reopen appearance; rotations; invalid metadata; missing fonts; overflow recovery; and frontend cache/history isolation. Run the existing suites and production build.
