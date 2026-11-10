# Fit paragraphs within their original clipping paths

The Word producer corpus still rejects `Design` to `Design this` in table cells,
although an explicit 80-point rectangle succeeds. Callout checkpoint acceptance
confirms both original Word table paragraph cases remain refused on current code.
The earlier diagnosis is in artifacts/callout-editing-20261028/table-fit-diagnosis.md.

The fitting search finds its widest clear endpoint using page/obstacle geometry.
Its later layout check enforces exact source clipping. The endpoint can therefore
fall outside the clip even though a smaller useful rectangle fits. The search
then abandons that direction. Vertical expansion has the same inconsistency.

## Decision

Include `Prepared::check_context_area` in the anchored rectangle's admission for
current-draft fitting. Retain the strict layout/publication check. Preserve the
current draft's top and left/center/right anchors, native coordinates, obstacle
proof, font metrics and search bounds. Test fresh initial-area containment too;
only change its separate source-baseline search if a violation is reproduced.

Alternatives considered: intersecting the page box with an exposed context clip
would duplicate context interpretation and complicate transforms; relaxing the
publication tolerance would admit clipped output. Reuse the existing authoritative
containment check instead. This is the coordinator's implementation choice under
the user's standing authorization for autonomous fixes.

## Acceptance and limits

Synthetic clipped PDFs must fit wider and taller drafts when a valid rectangle
exists, preserve center/right anchors and transformed/rotated geometry, and still
refuse impossible or initially out-of-clip requests. Save/reopen/re-edit must
retain exact source text order and surrounding text. Verify both original Word
files with two saved edits, independent raster/text/font/link/tag-graph checks,
and current desktop fit/preview/undo/save/reopen behavior. Recheck the full
36-workflow producer matrix and relevant paragraph/context native regressions.

No clip semantics, glyph admission, layout metrics, UI contracts, dependencies or
original PDFs change. Preserve existing artifacts and packages. One final scoped
review, inline implementation, no commits or publication. Broader script/font,
hardware/performance and commercial parity remain separate requirements.

Investigation ruling: a changed first-line ink ascent can move the baseline while
the requested top stays fixed (Courier `AA BB` to `AA BB CC`: 0.156 points).
That is retained as a separate layout-fidelity gap, not relabeled rounding or
claimed fixed here. The clipping anchor cases hold glyph heights constant.
Fresh tight-clip initial suggestions passed without a source change, so their
existing source-baseline logic remains unchanged.

Independent audit integration requirement: transformed clip replay can hide
generated text in MuPDF although native PDFium validation passes. Correct the
publication recipe while preserving source clip meaning, legacy saved-envelope
recognition and bounded repeated editing. The fitting change cannot be accepted
until all transformed outputs pass the independent readers and raster checks.
