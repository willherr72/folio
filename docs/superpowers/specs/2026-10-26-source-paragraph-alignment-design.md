# Preserve fresh paragraph alignment

Coordinator decision under the user's standing autonomous-development request.
Use the existing native Rust inspection/layout pipeline and desktop editor.
Source-order publication is being finalized separately; do not change product
sources until its active regression run and fingerprint check finish.

## Observed problem

The supplied quiz's three-line instructions are visibly centered, with a red
highlighted phrase. The current packaged editor successfully inserts text into
that phrase, preserves its original font/color/highlight, reading order and
neighboring pixels, but reflows the paragraph left aligned. Evidence lives under
artifacts/paragraph-order-20261026/highlight-desktop. The independent report lists
source/edited line positions and keeps this gap separate from its passed checks.

Fresh styled inspection calls paragraph_baseline_mode with mixed=true, which
skips the existing uniform alignment inference entirely. The frontend therefore
defaults to Left. Initial bounds also pad only the right edge; simply enabling
Center or Right would move the alignment anchor. Automatic fitting currently
grows only toward the right, likewise moving a centered/right-aligned anchor.

## Chosen behavior

Fresh native paragraph inspection should propose Left, Center or Right when
multiple distinct-width source lines provide unambiguous geometric evidence.
Use each run's own physical font advance/ink and checked glyph positions, grouping
pieces by their actual baseline. This includes mixed fonts/sizes/colors and
verified highlights. Do not derive alignment from serialized hints unless the
existing exact replay proof succeeds. Equal-width rows, single rows and conflicting
edges/centers remain ambiguous and retain the existing default/manual controls.

The inferred frame retains the measured source center/right edge (or left edge).
Measurement padding must extend symmetrically for Center and to the left for
Right. Account for actual ink overhang and verified highlight padding, using the
same metrics as layout/publication. Derive tolerances from bounded physical
measurements, not a quiz-specific offset. Retain source first baseline and existing
baseline spacing. Mixed-size rows derive their spacing ratio from the larger em
of each adjacent row pair, matching layout; a first-piece-only ratio otherwise
widens and rewraps unchanged paragraphs. Inconsistent ratios remain ambiguous.
Existing saved paragraphs restore their authored alignment
and area exactly; explicit user alignment/position changes remain authoritative.

Initial areas include the measured last-row descent at the original width when
the necessary vertical space is clear. A provisional quarter-em source bottom
must not force an unchanged three-row paragraph into a wider two-row layout.
The existing full-font minimum baseline separation remains in force for saved
replay compatibility: source leading below that minimum is still clamped. Both
the tight18pt Courier case (18.112pt existing minimum) and representable19pt gaps
remain tested. Exact dense leading is an outstanding typography fidelity item,
not a claim of this alignment checkpoint or a dropped part of the broad goal.

Fit area to text preserves the current horizontal alignment anchor: left edge
for Left/Justify, center for Center, right edge for Right. It may widen into clear
space and extend downward, retaining current source-baseline anchoring for fresh
suggestions. Both sides of a centered expansion and the left side of a right-
aligned expansion must respect page bounds and neighboring text/graphics. Keep
the safe-width-plus-height correction and the bounded search; failure must leave
the current area/draft untouched. The fit is a convenience, not a claim of globally
minimal area or a replacement for publication's independent collision proof.

Uniform justification inference remains supported. Mixed justification requires
its own complete evidence; do not treat matching outside edges alone as proof of
expanded word gaps. No broader script/font/selected-operator admission is implied.

## Alternatives considered

1. Frontend-only alignment guessing: rejected because it lacks original glyph
   positions and would let UI and saved-layout geometry disagree.
2. Require manual alignment for every existing paragraph: retains the observed
   loss of formatting on ordinary edits and does not meet the usability goal.
3. Shared native row measurement plus alignment-aware area fitting: chosen. It
   keeps inference close to checked source data and supports all existing clients.

## Boundaries and verification

No changes to original PDFs, passwords/rights, source ownership, font substitution,
publication's glyph/pixel checks. Saved layout semantics remain authoritative;
the review correction below explicitly versions the new highlight alignment policy. No new dependency.
Retain operation, source, fragment, probe and area budgets. Refactor only the
measurement/fitting units required for this behavior; keep the uniform API's
existing inference and selected-user behavior covered.

Behavioral RED then GREEN: fresh uniform/styled/mixed Center and Right; Left and
ambiguous controls; multiple pieces per row; trailing spaces; mixed sizes/colors;
negative ink bearings; verified highlights; crop/rotation/UserUnit. Inspect,
suggest, edit, fit, save/reopen and repeat should retain their intended anchors.
Fitting tests must include blockers on each side, page limits, combined width/
height and mixed-height first-line changes. Preserve surrounding pixels and
source bytes. Review one final scoped implementation, correct findings inline.

Fresh packaged Windows acceptance: edit the quiz's highlighted instructions
without manually selecting Center, verify the inferred control and native preview,
Apply/Undo/Redo/save/reopen/re-edit. Independent readers check reading order,
centered output rows, original fonts/colors/highlights, baseline and surrounding
pixels. Retain prior source-order checkpoint and failed evidence. No parity claim.


## Review correction: text anchors and highlight reservation

The final review traced asymmetric edge highlight padding into horizontal layout:
source inference measures text, but padded layout moves a marked row to align its
background. A single frame offset cannot preserve differently marked rows. Reproduce
with distinct-width unbreakable Courier rows and left/right overhang, for Center/Right.

Use separate measured text alignment and highlight clearance for fresh marked drafts.
Reserve bounded left/right background insets in the outer frame; use the inset text
frame for alignment and wrapping, padded glyph metrics for vertical clearance,
and independently verified native rectangles for actual highlight containment. Initial
outer framing expands around the source text frame, and Fit preserves its content anchor.

This requires an internal version-4 saved layout flag. Existing versions1/2/3 and their
absent flag keep the previous padded alignment algorithm, including unchanged re-edit.
New drafts persist the flag and explicit horizontal text insets; replay independently remeasures both sets of metrics,
checks the version, verifies exact fragment origins and pixels before accepting memory.
No new frontend request setting, dependency, or relaxed publication proof is required.
The explicit schema exception is necessary to preserve previously saved typography.

Acceptance: original glyph Center/Right anchors with asymmetric marks, native publication,
mark geometry, save/reopen/repeat pixels, legacy v2/v3 policy preservation, forged flags,
plus corrected quiz and affected gates. Correct inline; no second review round.

The outer frame reserves the maximum verified horizontal padding before wrapping;
wrapping must not subtract that padding a second time. Source installation retains
the text frame before union with marks, so a mark on the widest row cannot shift it.
The initial descent extension only accepts a layout with at most the source row count;
otherwise bounded width fitting runs first to avoid turning one source line into two.

The reservation is a saved frame setting, not a derived property of the remaining
painted marks. Keep its checked horizontal insets when deleting the last highlighted
word; publication still removes every unused background template. Verify insets by
exact native replay, with finite bounds and version admission. This also preserves
anchors through formatting/style palette normalization. New layout uses signed
remaining width within the existing EPSILON wrapping tolerance to keep exact Center/
Right anchors; old versions retain the clamped equation. No tolerance is enlarged.
