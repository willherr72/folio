# Embedded subset source eligibility

An eligible single PDF text show must remain editable when PDFium adds a reading
space that the original embedded font cannot encode. Actual missing glyphs in
source or replacement must remain unsupported. Preserve raw native text as stale
selection identity, paragraph binding, neighbor and glyph evidence.

Use the existing bounded source operand mapping. Factor property/text validation
into one helper used by raw snapshots and source-aware local editing clones. Only
recompute a local clone when independently verified operand text exists. Preserve
font-proof failures; re-evaluate all render-mode, opacity, transform, font and
text limits rather than clearing an error string. Listing uses local eligibility;
no-op and post-edit admission use it too. Raw snapshots remain unchanged for
paragraph/group code and preservation comparisons. No IPC or metadata change.

Alternatives rejected: stripping reader spaces cannot distinguish actual spaces;
clearing U+0020 errors by message can mask other restrictions; changing all raw
snapshots would change paragraph/group semantics. This scoped design fixes the
single-run path while retaining evidence for later paragraph compatibility work.

Acceptance: verified CID subset without a space mapping, generated separation,
original-font edit through both APIs, rotations, saved/reopened/re-edited content,
unchanged font bytes, neighbor pixels, source and source file. New actual spaces
remain a precise missing-glyph refusal. Transparent/stroked/singular and
unverifiable-font cases remain refused. Native/group/paragraph regression plus
Windows real dialog and independently rendered saved references. One final scoped
review and correction pass; no commit, release or parity assertion.
