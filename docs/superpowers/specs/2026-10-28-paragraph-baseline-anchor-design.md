# Stable paragraph baseline anchors

Existing PDF text edits should retain the first line's baseline unless the user
moves it. The accepted clipped-fit investigation demonstrates Courier `AA BB`
becoming `AA BB CC` at baseline599.844 instead of600: the current first baseline
is frame.top minus replacement ink ascent. Changing letters therefore moves text.

The user authorized autonomous implementation and opted out of approval pauses.
The decisions below are the coordinator's, not additional user promises. Use the
existing isolated worktree, inline implementation, and one final scoped review.

## Approaches and choice

Using nominal font ascent would undo tight-leading fidelity and still move mixed
font/size paragraphs. Inferring movement from a changed top edge cannot distinguish
explicit positioning from fitting a taller glyph. Automatically painting beyond
the submitted area would make collision/clip/frame controls misleading.

Choose an explicit optional baseline anchor in displayed physical page coordinates.
It is placement intent, not source/font authority. The source selection, exact
context, bounds, font proofs and independent publication checks remain required.
Keep frame-based legacy behavior for requests without this field.

## Contract

- Native `StyledParagraphSelection.baseline_anchor: Option<Point>` serializes as
  `baselineAnchor`, the first logical line's baseline point. Fresh selection uses
  the first source run; validated memory uses its replayed first logical line,
  including leading blank lines. The displayed point is before editor rotation.
- `StyledParagraphLayoutRequest.baseline_anchor: Option<Point>` is optional and
  defaults to absent. Public values are physical points, worker values raw page
  units. Convert at the existing facade; preserve unchanged source values across
  UserUnit roundtrips. Point coordinates must be finite and bounded.
- A supplied anchor determines only raw PDF baseline Y. Horizontal alignment
  remains determined by the paragraph frame and alignment. First-line ascent,
  descent and following-line spacing still use measured draft glyphs/styles.
  Reject ink reservation exceeding the area instead of clipping or moving it.
- Fit keeps the anchor and left/center/right alignment. At each candidate width,
  measure the first row and grow the frame top if required; retain strict page,
  context and obstacle admission. Grow width/bottom using existing search. Never
  silently shrink an already sufficient area or relax clipping to obtain a pass.
- Persist a new optional raw `first_baseline` in disposable paragraph records,
  version7 when used. Old versions must reject the new field, not reinterpret it.
  Replay with this baseline, then prove exact fragments/state/font bindings and
  raster equality before restoring. No-op migration of old valid records must
  preserve actual pixels and baseline; forged/nonfinite/out-of-area hints cannot
  authorize an edit. Leading blank lines must survive saved re-edit.
- Desktop draft history owns the anchor with the area. Text/style changes and
  fitting retain it. Position-only changes translate it by displayed dx/dy;
  resizing retains it. Fit uses an explicit transaction intent so a top-edge-only
  expansion cannot look like movement. Undo/redo restores both together. Reset
  position translates back without discarding current text/style edits.
- CLI/MCP paragraph preview submits the inspected anchor by default. Explicit
  input bounds translate it by their displayed origin delta before fitting;
  the existing optional bounds mean an explicit placement request. Top-level,
  occurrence and reusable-form routes use the same layout and physical facade.

## Verification and limits

Reproduce the actual current baseline shift before implementation. Test taller and
shorter replacement ink, explicit translation, fit expansion and blocked growth,
mixed sizes/highlights, blank first lines, all page rotations and UserUnit1.7.
Check saved re-edit and exact no-op raster, source bytes, neighboring glyphs/pixels,
font/link/tag preservation, and legacy/forged metadata behavior. Editor tests
must distinguish Fit, Move, Resize and history. Current CLI/MCP and Windows
acceptance must exercise a changing-ascent edit, not only constant-height words.

No scripts/shaping expansion, new dependency, source PDF mutation, optimized
package overwrite, or commercial-parity claim. Native gates run serially with
one locked/offline Cargo, jobs1; never change tested inputs during a live gate.
