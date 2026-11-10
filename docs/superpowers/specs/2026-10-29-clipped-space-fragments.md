# Clipped paragraph space fragments

Folio must preserve spaces between independently styled fragments in clipped or
tagged paragraphs. A space-only fragment must not be treated as missing visible
text when the saved native object genuinely paints no ink. Original/replacement
visible text must still fit its original clipping path.

Evidence: `artifacts/word-highlighted-reflow-20261029/fragment-controls-1`.
The same diagnostic Word PDF rejects unchanged/replacement runs with a separate
space, but accepts moving the space into either neighboring style. This is separate
from the original Word highlight's partial ink enclosure, which remains unsupported.

Chosen approach: preserve the ordinary `check_ink` path; admit a narrowly proved
space-only native object when its independently decoded source contains only
nonempty ASCII spaces, native evidence establishes no painted area, and its finite
origin remains in an admitted clip. Do not trust extracted blank text alone or
accept arbitrary missing glyphs, other whitespace, visible custom-space glyphs,
nonfinite coordinates, clipping text modes, or outside-clip placement.

Carry this proof through original source and saved candidate checks as required
by reproduced behavior. Prefer a private helper/optional source value over a public
request field or metadata authority. Preserve tags, neighboring ink, and source
bytes. No dependency, renderer, package, version, or public API change.

Acceptance: native regression first fails on the current implementation; corrected
preview, save/reopen and repeated editing retain exact text and unchanged outside
content. Negative controls retain rejection of visible clipped/missing glyphs and
unproven blanks. Existing context/highlight suites pass. A fresh CLI candidate must
rerun the real control before claiming runtime resolution. This does not close
the original partial-highlight or resource-growth gap.
