# Fit the paragraph area to the current draft

Longer edits currently fail layout unless the user changes numeric dimensions.
Add an explicit **Fit area to text** action to the styled paragraph dialog. It
measures the current draft with its resolved original/substitute fonts, sizes,
colors, alignment, spacing and verified source highlights. No typography change
or source mutation is implicit in fitting.

Reuse bounded native area search from initial inspection. Keep raw PDF top/left
anchor fixed; try widening, extending downward, then both. Every returned endpoint
must fit measured layout/highlight padding, stay inside the crop page and avoid
unselected text/graphics. A quarter-turn page can change displayed Left/Top when
the raw rectangle grows; its text anchor stays fixed. Search is a useful bounded
suggestion, not a globally minimal layout proof. Failure keeps the draft and area.

Split shared request admission/measurement from layout so a draft that overflows
can still be measured without weakening validation. The new native API returns
only an AnnotationRect, not a published document. Existing preview/publication
checks remain authoritative for exact native glyphs, paint, neighbor pixels,
comments, source restrictions and saved-memory proof. Fitting never grants Apply.

The UI sends the complete current request; a changed result becomes one area
history step and invalidates any earlier preview. An unchanged result retains
history and a valid preview, with feedback. Invalid numeric fields or unavailable
fonts disable fitting. Temporary font leases survive pending native work. Late
results after editing, selection adjustment, source/adapter change or unmount are
ignored; changing just the text selection must not lose the current selection.

Acceptance: original/substitute fonts, highlights, crop/all rotations, already-fit,
blocked and no-room cases, stale source, invalid styles/glyphs; source immutability;
draft/history/preview and asynchronous races; Windows fit/preview/apply/undo/save/
reopen, independent saved-output comparison and scoped review. Keep limitations
and prior executables; do not claim Adobe/Foxit parity.

Native testing exposed an existing highlighted imported-font measurement gap:
actual native ink can exceed program bounding boxes (the authored18pt accent
case extends0.21954pt above the predicted mark). Do not enlarge containment
tolerances. Add independently measured native ink for marked imported fonts in
new/changed drafts. Version3 saved paragraph hints record this measurement mode;
legacy version1/2 no-op re-edit retains legacy metrics. The flag selects an
independently reproducible algorithm, never trusted coordinates or font authority.
Fresh/current font resources must generate scratch native glyph measurements,
and saved replay still verifies exact origins, bytes and full-page raster.
Retain legacy saved fixtures and test new-mode replay after font-registry release.
