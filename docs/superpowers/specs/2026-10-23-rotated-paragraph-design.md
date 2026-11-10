# Paragraph editing on intrinsically rotated pages

The next compatibility cycle lets existing uniform paragraphs remain editable after
page rotation is saved into a PDF. The user authorized autonomous iteration; no new
approval is needed. Preserve all existing work and portable checkpoints.

## Coordinate contract

The public selection rectangle, Left/Top, width/height, preview, text-piece targets
and drag coordinates use the source page as displayed, including intrinsic /Rotate
but excluding additional workspace rotation. The native writer converts that area
to raw PDF coordinates before wrapping. Paragraph lines retain their original raw
horizontal orientation, font, size and color. A quarter-turn page therefore has
vertical displayed text; its displayed area height controls line length. Rotating
text independently, mixed formatting and arbitrary transformed paragraph text are
separate work. Existing single-run capabilities remain unchanged.

Retain the source /Rotate, CropBox, original byte immutability, permission policy,
overlays, source identity/history and candidate lifetime. Convert rectangle corners
and take minima/maxima; do not assume the first transformed corner is top-left.
Validate the requested area in displayed space, then wrap and check ink in raw space.
Compare unselected glyphs and pixels using the correctly rotated change area.

## Alternatives considered

Removing the rotation guard alone leaves incorrect placement, dimensions and pixel
masks. Temporarily clearing /Rotate would require new UI geometry/preview protocols
and overlay transforms. Converting rectangles at the existing native boundary keeps
the UI's established displayed-space contract and preserves source orientation.

## Acceptance

- All four rotations with ordinary and nonzero CropBox: inspect, replace with wrapped
  lines, move, save/reopen and edit again; exact font/direction and neighbor geometry.
- Reject overflow, outside-page areas and neighbor collisions without source changes.
- Protected rotated source retains credentials/rights; rotated overlays roundtrip.
- Independent MuPDF/pypdf extraction, glyph origins, direction, crop/rotation and
  unchanged pixels outside separate old/new areas; original hashes unchanged.
- Actual isolated Windows saved/reopened rotated PDF: select, preview, move, apply,
  undo/redo, save/reopen, comments retained. Existing frontend and relevant native
  suites/builds pass; one fresh review before recording completion.
