# Word italic highlights bound to character advances

Goal: edit the original authored Word paragraph in
`artifacts/word-highlighted-reflow-20261029/inputs/highlighted-word.pdf`, including
longer highlighted text, wrapping, save/reopen and faithful re-edit. Its unchanged
bytes remain the acceptance source. Enlarging its rectangle is only a diagnostic.

Word's mark matches the text advance more closely than italic ink: the final w
extends about 0.64pt past the yellow rectangle, while the source advance differs
from its right edge by about 0.034pt. The y extends about 0.027pt below it. Generic
partial graphic coverage must continue to refuse. Raising global EPSILON or
clamping negative ink padding would erase that distinction and distort replay.

## Source authority

Keep the existing fully enclosing highlight route unchanged. A separate advance
route may admit an opaque, noncompeting rectangle painted before one complete
selected text object. Require exact decoded source-to-native glyph mapping, no
trimmed outer whitespace, one baseline, positive monotone character advances,
ordinary reproducible text state, and every visible glyph overlapping the mark.
Any overlapped visible glyph in another object rejects this route, even if selected.
The mark's horizontal edges must match the complete run's verified origin/advance
envelope within 0.005 em. Vertical ink underhang is bounded by the same 0.005 em;
positive vertical padding is at most 2 em. Baseline must be inside the mark.
These are local source-frame criteria, not changes to generic overlap tolerances.
Source proof, page preflight, source paint binding, tag/clip ownership, collision,
and final text/raster checks remain required. No requests or stored hints establish
fresh source ownership.

## Geometry and replay

Add an explicit optional character-cell policy to Background, leaving old ink
padding records unchanged. The new policy stores dimensionless left/right offsets
from character origin/advance and below/above offsets from actual glyph ink.
Horizontal offsets are bounded to +/-0.005 em; vertical offsets to [-0.005, 2] em.
The legacy padding field is zero in cell mode, preventing ambiguous dual policies.

For each visible glyph, derive its cell paint box from its actual origin, verified
advance, ink and style size. Union these boxes for each contiguous marked range on
each line. This preserves the existing ink-relative vertical behavior while
handling italic horizontal overhang. Scaling offsets with font size prevents a
fixed underhang from collapsing tiny text. Native publication remeasures glyphs
and verifies font/advance/origin before painting. Layout reserves the union of full
ink and cell paint, so a smaller highlight never shrinks text collision bounds.
Blank-only fragments paint no mark. Other text and marks retain collision checks.

Persist the new policy only in version 8 hints. Versions 1-7 reject it. Replay
requires reproducing exact paint blocks from native glyphs and verified advances,
unique paint ownership and no unselected-glyph coverage. Changed or forged records
never authorize arbitrary removal or bounds. Existing legacy replay stays exact.

## Acceptance and scope

Native regression must fail before implementation. Test source edge mismatch,
partial run, foreground/competing graphics, another text owner, multiple baselines,
nonfinite/degenerate metrics, tiny/large font sizes and legacy hint rejection.
Verify original Word longer-text wrapping, save/reopen/re-edit, font/color retention,
all unselected glyphs, source/request-bound pixels, links, second page and nonempty
tag graph through independent readers. New source discovery initially requires one
complete text object; fully enclosed multi-object highlights retain their existing
route. Explicit paragraph formatting can split the admitted range into new styles.
No resource-budget changes, renderer changes, dependencies or package overwrite.
The prior resource-growth failure remains open and must not be relabeled.
