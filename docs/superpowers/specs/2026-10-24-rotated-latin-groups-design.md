# Rotated Latin grouped editing

Continue the existing-text reliability objective by admitting verified accented
Latin groups and single-character pieces on intrinsically rotated pages. The
current group path reuses an ASCII-only, page-wide reader workaround. Its global
conditions unnecessarily reject selected text whose embedded encoding, native
direction, geometry and surrounding appearance can be proved directly.

Coordinator choice under standing autonomous authorization: keep the shared
reader helper unchanged. For rotated grouped editing retain its 64-object and
4096-character page bounds, but validate the selected pieces in unrotated source
axes. Require positive axis-aligned finite transforms, attributable contiguous
characters, a common finite baseline, nondecreasing native x origins and positive
overall advance. Existing Latin/encoding/font-resource checks, exact no-op output,
merged glyph comparison, six-pixel neighbor exclusion, and final crop/collision/
outside-pixel checks stay mandatory. Unselected directions or scripts must not
alone reject a supported group; their glyphs and pixels remain protected.

Alternatives: widening the global reader workaround would affect selection and
copy on unrelated pages. Dropping all direction checks would admit ambiguous
counterrotated/mirrored text. Selection-specific validation improves editing while
keeping these boundaries explicit. No new shaping, bidi, mixed formatting, nested
form groups, font substitution or non-Latin selected-text support is claimed.

Acceptance: embedded accented phrases at seven split boundaries through all four
page rotations, with crop and save/reopen; exact no-op source pixels/objects and
unchanged source bytes; original font and neighboring content preserved. Verify
single-character pieces, unrelated differently oriented text, and retained refusals
for selected mirrored/sheared/vertical/backward/baseline-changing text. Independently
compare exported outputs and exercise the actual Windows selection/edit lifecycle.
