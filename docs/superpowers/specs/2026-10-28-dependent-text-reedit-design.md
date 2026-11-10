# Saved dependent text re-editing

The preceding milestone is complete. A new native regression now reproduces saved
tagged text growth failing with the surrounding-text preservation error after the
external font has been removed. The positioned route rewrites the selected show
without compensating its cursor advance; the following show moves and fails native
validation. This is a compatibility defect, not a request to reflow a paragraph.

Use the existing private source-bound text plan for source-font edits on complete-
source transactions that already require positioned preservation. Preserve original
Tj/TJ/quote semantics, font resources, text line matrix and inherited state. Derive
the old/new advance difference from verified font widths, character spacing,
effective single-byte code32 word spacing and retained TJ gap totals. Apply a numeric
TJ correction immediately after the selected show, before any font/state change.
Merge it into an existing adjacent numeric-only TJ to avoid growing correction chains.
Do not infer text advance from ink bounds or regenerate unrelated page operators.

Outer and inner clipping/tag publication bind the exact private plan just as explicit
font substitution does. No caller-supplied operation recipes or metadata gain authority.
Preserve glyph support, stale-selection, clipping, overlap, permission, native style,
other-run and outside-pixel checks. Keep all existing limits. The ordinary native
route and legacy isolated route remain when positioned preservation is not required.

Alternatives considered: regenerate all text through the renderer (loses protected
operator context); force paragraph reflow (changes the requested single-run behavior);
modify the positioned serializer without binding the context proof (would fail exact
publication). Reusing private plans keeps authority and cursor arithmetic together.

Acceptance needs saved substituted-font growth without the registry, source-font
growth/shrink/repetition, independent expected glyph advance checks for spacing and
quotes, following-run/line preservation, complete tagged graph and second-page checks,
and overlap/stale refusal. Run affected native/automation checks, build and inspect
fresh saved outputs, then obtain the user-authorized single final scoped review.
No release, printing, general shaping or commercial parity claim. Original documents,
accepted package and inherited dirty worktree remain intact; no commits.
