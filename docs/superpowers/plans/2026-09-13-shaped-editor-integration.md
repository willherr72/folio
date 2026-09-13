# Shaped editor integration implementation plan

Continue the user-approved shared-layout integration in the existing isolated
shaped-text worktree. Architectural extension of #14; explicit opt-in custom-font
single-line shaping preserves legacy document behavior.

## Contract and responsibilities

- TextOverlay gains optional shaping: {version:1,direction:"auto"|"ltr"|"rtl",ligatures:boolean}.
  Absence is legacy. FontId is required for shaped text. Native serde rejects unknown versions.
- Native prepare_text_overlay command takes overlay and returns an immutable serializable
  result: {preview:NativeTextPreview,origin:[number,number],bounds:AnnotationRect,characters:PdfTextCharacter[]}.
  Origin is native semantic PDF baseline origin. Bounds/characters use local editor coordinates
  before x/y and clockwise overlay rotation. Preview source axes remain PDF axes; SVG parent
  translates by (-originX, fontSize+originY) then flips Y. Preview includes requested color.
- Native agent owns Rust integration: schema, shared preparation, command, persistence,
  export validation, default feature, shaped coverage and save/reopen/recovery regressions.
  Keep exact font program ownership and resource validation; never trust serialized glyph input.
- Root owns TS schema, bounded/coalesced preview cache, font leases, late-response handling,
  TextOverlayPresentation and geometry/search integration, tests, docs and final validation.
- UI agent owns OverlayProperties shaped controls, IME draft editing and focused interaction
  tests; coordinates FontPicker compatibility without changing existing-run substitution.

## Execution

1. Write focused failing tests for native shaped appearance roundtrip/refusals, JS result
   ownership and stale response, and UI composition boundaries. Use test-first skill.
2. Implement the contract with native authoritative validation. Opt-in shaping uses exact
   outlines and semantic text. Reopened metadata regenerates and verifies appearance/resources.
   Existing boxes and unsupported mixed/RTL separator/capacity guards remain.
3. Add bounded preview cache keyed by exact font/text/size/shaping/color; native preparation
   holds font ownership until response settles. Only current requests can update presentation.
   Geometry/search use native cluster rectangles and exact logical scalar sequence.
4. Add explicit custom-font shaped mode. Composition stays local until compositionend;
   grapheme deletion uses platform text input. Clear status explains pending/unsupported output.
5. Verify preview/export at rotations, undo/redo, recovery and font lifecycle with focused
   integration tests, full native suite(serial linkers), bounded-worker frontend suite/build,
   browser smoke and independent review. Record limits, commit/push and update #14.

Ruling: prior explicit approval covers implementation and repo tracking; no repeated approval
pause. No automatic script widening for legacy text. Actual desktop release only after the
integration acceptance checks pass; all changes remain reviewable in the repository.

Completed implementation and automated acceptance. See [integration report](../../shaped-editor-integration.md).
Review rulings: search consumes prepared response before cache release; cache evicts idle
payload before admission and exposes Retry; busy properties disable/discard local drafts;
font picker validates authored size; cmap preflight bounds callbacks and linear lookup work.
Native source crop/rotation and unsupported-draft recovery matrices were added to cover
review gaps. Canvas glyph caret editing and desktop/manual release acceptance remain
explicitly outside this checkpoint's claims; existing Content input handles editing/copy.
