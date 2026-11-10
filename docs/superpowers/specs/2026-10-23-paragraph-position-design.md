# Position existing uniform paragraphs

The autonomous quality loop now exposes a capability already accepted by native
`preview_text_paragraph`: a replacement area's x/y position. Continue the approved
uniform font/size/color scope. Native source rotation limitations remain unchanged;
workspace rotation is retained on adoption, while this dialog uses original page axes.

## Product behavior

After Check selection, show Left (pt), Top (pt), width and height fields. Left/Top
are distances from the original page's upper-left corner. Numeric drafts retain
invalid/empty input; disable Preview and explain that the complete rectangle must
fit the page. A valid change invalidates and releases any prior candidate exactly
as text/size edits do today. Reset position restores the inspected x/y without
discarding edited text or width/height.

In Original view, drag the outlined area to move it without resizing. Clamp the
complete rectangle to the page. Show a local outline while dragging and commit
position only on pointerup; Escape, pointercancel or lost capture restores the
previous position and preserves the candidate. A drag with no position change is
a no-op. Edited view retains a static outline; use Original to adjust placement.
Left/Top fields provide the keyboard-accessible equivalent, so the pointer-only
SVG affordance is decorative to assistive technology. The existing comparison
and zoom controls are disabled during a drag, as are field changes, Check/Preview
and Apply. Cancel remains available. No global document/history change occurs
before native preview and Apply.

## Design choices

Use a small rectangle-clamping helper and an isolated SVG movement component.
The existing added-text area's resize helpers require36pt minimum dimensions and
TextOverlay state, so do not force paragraph bounds into that unrelated model.
Use SVG screen-coordinate inversion for pointer mapping at any preview zoom.
Keep gesture state local, pointer capture and a generation-safe cleanup boundary.
Reuse the existing candidate ownership and native writer unchanged unless fidelity
tests expose a defect. No font substitution, automatic paragraph pushing, mixed
formatting edits or new native source-rotation claims.

## Acceptance

UI RED/GREEN tests cover numeric drafts, candidate invalidation, reset, drag commit,
cancel/no-op, clamping and disabled Apply during gestures. Native paragraph tests
cover moved text, unchanged neighboring text/source, crop origin, protected rights,
out-of-page and collision refusal. Actual browser checks cover real pointer capture
and zoom. Actual Windows workflow moves a paragraph, previews, applies, undoes/redoes,
saves/reopens and preserves workspace rotation. Independent MuPDF/pypdf verify text,
placement and pixels outside old/new regions. Review the whole milestone once.
Preserve verified portable0.15.0-dev.1; package a later combined checkpoint separately.
