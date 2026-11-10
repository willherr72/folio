# Saved comment controls on the page

The prior goal turn made progress: portable 0.18 was built and verified. Continue the user's authorized autonomous quality loop without routine approval questions. Use the current React/SVG and native immutable-source architecture; preserve all existing work and ready packages.

## Approach and behavior

Use transparent SVG hit targets over native rasterized notes, with a dashed destination outline during movement. Commit through the existing native comment edit command on pointer release. This keeps custom icon appearances and metadata intact. Converting notes into workspace overlays would discard fidelity; rendering/replacing a native PDF on every pointer event would add latency and unnecessary source ownership. These alternatives are rejected by the implementer.

In Select mode, click focuses/selects a saved note, double-click or Enter opens the existing editor. Drag moves its outline, clamped inside the page, and release applies one immutable source replacement / undo entry to every shared source page. A click without movement creates no edit. Arrow keys move a draft by 1 point (Shift: 10); Enter applies and Escape cancels. Original icon remains visible until the native change is rendered. Provide visible instructions and preserve focus after successful movement. Errors keep the source and explain the failure.

Locked/read-only notes remain focusable/readable; movement follows both native reason and page annotation permissions. Hidden/invisible/no-view/toggle-view notes remain in Review but have no page target. Native metadata explicitly reports whether page hit testing is appropriate. Keep text selection and overlay controls working outside note targets. Non-Select tools must not be intercepted.

Cancel uncommitted pointer/keyboard gestures on Escape, pointer cancellation/lost capture, focus loss, unmount, tool, source, page geometry/rotation, zoom, or blocked state changes. Competing pointers cannot complete a gesture. Stale native results must not attach to changed documents; unused candidates are closed. Exact unchanged Contents representation should be preserved during movement, resolving the previously recorded normalization gap.

## Verification and limits

Tests cover all four workspace rotations, zoom, page bounds, keyboard drafts, no-op/cancellation, permissions/hidden notes, source replacement and one-step undo. Native tests cover visibility flags and unchanged/missing Contents. Run real Windows pointer/keyboard, undo/save/reopen and independent metadata/pixel checks on cropped/intrinsically rotated/custom-appearance fixtures. One fresh focused review and one fix pass at completion. No version bump, publishing, commit, new dependencies or ready-package replacement. Rich threads, mixed-format text and existing broader parity gaps remain.

## Review and acceptance corrections

Rendered hit bounds are separate from editable PDF rectangle bounds: intrinsic
NoRotate appearances stay upright about their transformed upper-left anchor, and
PDFium's pinned default Text icon uses20pt geometry. Drag/keyboard changes translate
both rectangles while clamping their union to the page; native editing still accepts
original rectangle coordinates. Pixel regressions use the bundled PDFium at all
four intrinsic rotations with normal/NoRotate/NoZoom+NoRotate flags.

PDFium rendering was observed to resize default-note Rect and synthesize AP in its
runtime document. Composition now restores native Text Rect and AP absence/null
from immutable source data before finalization, matching retained annotation order,
subtype and decoded text. This preserves untouched annotations through render/save.
Focus restoration is discarded after deliberate focus/tool/zoom changes. Loss of
effective annotation permission cancels an existing local draft immediately.
