# Mixed paragraph per-style formatting

Execute inline under the existing autonomous development authorization.

Goal: finish the explicit formatting portion of mixed-paragraph publication,
with source-bound derived palette entries for range-local font, size and color.
Faithful saved paragraph memory and GUI integration remain subsequent stages.

Each style change names a target style ID and an original inspected source style
ID. Resolve every change against the immutable source palette, never another
change. Existing entries may be overridden; new entries must be contiguous and
the entire palette is limited to 64. Reject duplicate targets, unknown source
IDs, invalid sizes/colors/fonts and unused malformed definitions. Defaults keep
existing requests compatible. A derived entry lets unchanged runs retain their
original style ID. Sizes are effective PDF point sizes, colors are #RRGGBB.

Keep original source bindings separate from resolved output styles. Imported
fonts use registry-owned Arc leases, verified TrueType metrics and the existing
full-font Identity-H writer. Install each unique font once, refresh resources
between installations, and preserve existing keys. Reopen and verify program
identity, encoding, color, size, glyph positions and unchanged content. Keep
current unshaped Latin paragraph admission; do not broaden script support.

1. [x] Add failing integration tests for local overrides, order independence,
   immutable originals, malformed palettes, missing glyph substitution,
   multiple fonts/resource collisions and registry/source lifetime.
2. [x] Add DTO and shared resolved-style planner; measure source or substitute
   fonts consistently and reject invalid requests before publication.
3. [x] Emit resolved styles; embed unique fonts and verify reopened evidence.
4. [x] Run focused native regression suites and independent export checks,
   request scoped review, resolve findings and record exact evidence/limits.

No GUI executable is produced for this unconnected API. No input PDF is changed.

Observed width-policy correction: the first actual imported-font publication
failed its glyph-origin proof because PDFium truncates fractional CID widths.
New mixed imported resources emit rounded integer widths, measured identically
by the planner, with a distinct Folio{hash}Paragraph PDF name. Legacy resource
names and fractional widths stay unchanged. The font program, CID mapping and
encoding remain independently verified; position tolerances were not widened.

Evidence: artifacts/mixed-paragraph-formatting-20261023/verification.json.
160 native tests pass, three ignored; 32 independent actual exports pass.
Scoped review has no unresolved demonstrated defects. Legacy/new same-program
coexistence passes after correcting the temporary fixture's collision area.
