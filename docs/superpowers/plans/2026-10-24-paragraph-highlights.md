# Highlight-aware paragraph editing implementation plan

> Execute inline using superpowers:executing-plans and test-driven development.
> User authorized autonomous project work; no routine approval gates. Preserve
> the existing isolated dirty worktree and all prior runnable artifacts.

**Goal:** Preserve the meaning and appearance of highlighted paragraph text when
words are inserted, deleted, formatted, wrapped, moved, saved and re-edited.

**Architecture:** Treat proven source highlight rectangles as attributed text
backgrounds, not as arbitrary graphics to ignore. Detect exact native rectangle
geometry and ordered glyph coverage; bind provenance to the source selection;
derive background attributes alongside original font styles. Emit replacement
rectangles behind their corresponding laid-out text and independently verify
both geometry and pixels. Unknown shapes/coverage keep existing refusal behavior.

**Tech stack:** Existing Rust/PDFium/lopdf pipeline, React/TypeScript attributed
editor, native and independent MuPDF/pypdf output checks. No new dependencies.

**Spec:** Design and invariants below; broader objective remains the product
quality goal in `docs/product-quality-loop.md`, not merely accepting one fixture.

## Design and invariants

The private quiz contains one opaque yellow axis-aligned filled rectangle, drawn
before the covered text. It is smaller than the full paragraph; merely exempting
it from collision checking would leave the wrong words highlighted after reflow.
Geometry-only inspection shows near-boundary whitespace/metric rounding, so use
actual visible glyph ink and explicit bounded tolerances, not text-run unions.
No private words or source copies belong in repository fixtures or logs.

Alternatives considered: leave graphics fixed (wrong highlight association when
text moves); blindly move every intersecting path (can damage art/tables); infer
an attributed highlight only from proven rectangles and glyph coverage (chosen).
This first detector is part of the full attributed-background pipeline. Do not
enable publication by weakening the existing blanket collision refusal before
the full source binding, rendering and re-edit verification exists.

Admission requires an opaque, unstroked, single closed axis-aligned rectangle,
normal blending under the existing page preflight, and paint order behind every
covered visible glyph. Curves, compound paths, strokes, images/forms, patterns,
clipping, transparency and foreground overlays remain unproven. Page-sized fills
remain backgrounds rather than styles. Avoid inferring semantics from color alone.

Coverage uses individual glyphs: classify fully inside, fully outside, or crossing
the border. Reject ambiguous partially painted visible glyphs, overlapping competing
marks, covered nonselected text, and text whose original resource/geometry cannot
be bound. Include spaces only by their adjacent attributed context; do not treat
an empty glyph box or a broad multiline object box as visible coverage.
Determine whether a source run is fully or partially highlighted explicitly;
partial runs need range attribution rather than changing the entire run's style.

Keep 4096 UTF-8 bytes, 64 styles, source restrictions, original expected text,
font proof, neighboring-object checks, immutable source bytes and complete saved
memory replay. Source highlight provenance is never trusted merely because
metadata says a background belongs to a style. A no-op must preserve original
appearance, including padding and hard/soft breaks; layout changes rebuild marks
around the corresponding text using verified source padding and line metrics.

Geometry admission additionally requires each source rectangle's covered ink to
share one baseline. A rectangle spanning several source rows may be block shading;
leave it unchanged until block semantics are supported. Wrapped replacement text
may produce one rectangle per resulting line. Derive nonnegative left/bottom/right/
top insets from actual source ink and preserve those in PDF points during font
changes. Reserve that space in layout, but use unpadded glyph metrics to construct
the final rectangles. Refuse spill onto differently attributed text or competing
marks; do not silently clip a source inset. Publication and saved-background
replay now use these helpers with independent native geometry and raster checks.

## Task 1: Native rectangle and glyph-coverage detector

- [x] Extract the existing solid-rectangle proof without broadening page-fill
  exemptions. Add an internal paragraph-highlight module for native rectangle
  discovery and bounded per-glyph classification.
- [x] Tests first: valid/translated/scaled rectangles; small/page backgrounds;
  curves/open/stroked/compound/translucent shapes; before/after paint order;
  multiline bounds whose individual glyphs do not touch the rectangle; visible
  glyphs crossing an edge; whitespace; partial source runs; competing marks.
- [x] Verify detector behavior with actual PDFium fixture objects and an opt-in
  private-quiz metadata-only probe. Do not copy source words or write a PDF copy.

## Task 2: Source-bound attributed backgrounds

- [x] Bind accepted rectangle painting operations and source glyph ranges to the
  checked selection. Cover all affected text; refuse incomplete ownership.
- [x] Preserve source byte ownership through logical joins/trims. Project complete
  paragraph highlights without repeated-text search; bridge whitespace only for
  the same original mark, and count styles after trimming.
- [x] Split otherwise identical font styles at background boundaries. Carry
  optional original background attributes/provenance through the native palette,
  immutable TS model, range replacement, resets and history. Font/size/color
  overrides must retain background association. No synthetic source authority.
- Native inspection installs only complete source-proven attribution atomically;
  unsupported/partial graphics retain the prior palette and graphics refusal.
  Initial measured areas include owned marks and reserve their padding. This is
  an area suggestion only; publication independently checks other graphics and
  accepts only source-bound or fully replayed saved background ownership.
- [ ] Test partial-run highlights, repeated words, spaces, mixed fonts, UTF-16
  selections, style-limit boundaries and dense request remapping.

## Task 3: Layout, publication and faithful memory

- [x] Emit measured rectangles behind the laid-out attributed spans; remove only
  proven original paint operators. Preserve unselected graphics and state.
- [x] Keep padding/baseline behavior across size changes, wraps and paragraph
  moves. Merge adjacent compatible backgrounds without joining unhighlighted gaps.
  Predicted layout, native scratch geometry and actual edited-output replay are
  verified for authored fixtures; broader cases remain below.
- [x] Verify saved background colors, geometry, ordering and actual raster before
  accepting metadata; reject forged/stale/overlapping background hints.
  Version 2 adds background templates and immutable style associations, retaining
  version 1 text-only compatibility. Reconstruct canonical paint blocks from
  verified emitted glyphs and bind unique native rectangles before full raster
  replay. Saved owned blocks are compacted on re-edit. If deleting a mark's last
  visible glyph leaves no painted evidence, remove that background association
  from saved styles; invisible metadata must not authorize a future highlight.
- [ ] Native tests across rotations, crop offsets, source and imported fonts,
  multiple highlights, partial selected ownership, neighboring art/text, source
  policy, saved no-op and repeated edits. Independent output reader/pixels required.

## Task 4: Connected acceptance and usability

- [x] Display proven highlight cues in the attributed input; native preview remains
  authoritative. No arbitrary background color editing until supported end-to-end.
- [x] Reflow authored highlighted paragraphs in Windows, apply/undo, save/reopen,
  re-edit and compare output independently. Retest private quiz with no export.
  Windows: four rotations, eight independently checked saved outputs. Native
  private probe: insertion with original font succeeds when the three same-row
  pieces are selected and the area is widened; no private export or mutation.
  This probe is API evidence, not a Windows private-document acceptance claim.
- [ ] Run relevant native and frontend suites, build, review and release timing.
  Preserve prior artifacts, record fingerprints/limitations and update user docs.

## Review focus

Paint order and transparent state, partially covered glyphs, broad multiline
bounds, background provenance after font/style remapping, unselected text inside
a mark, pattern/clipping false positives, saved-hint forgery and no-op fidelity.
