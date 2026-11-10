# Direct on-page paragraph editing

## Intent and authority

Continue the autonomous everyday PDF editor goal from the verified image-crop
checkpoint. Existing uniform and mixed paragraph editors already preserve fonts,
native paragraph reflow, highlights, source occurrences, candidate ownership and
saved re-editing. Their modal-only surface disconnects editing from the main page
and provides a drag-to-move rectangle but no on-page resize handles. Bring those
same capabilities into the main document workspace, with direct area manipulation,
native previews, page zoom/navigation and one final Apply/Undo transaction.
Standing user authorization opts out of questions/approval gates. Decisions below
are implementer choices. Preserve original files, dirty worktree and all artifacts;
no commits, releases or parity claim. Use one design check and one final review.

## Approaches

Creating a second paragraph editor around DOM text positioned over PDF glyphs would
duplicate attributed editing and misrepresent fonts that cannot be loaded by the
browser. Retaining the modal and adding only corner handles would leave the main
navigation and workspace disconnected. Choose the existing uniform/styled controllers
with a shared nonmodal workspace shell and source-axis preview binding. The text
entry and formatting controls sit beside the actual main PDF page at wide widths;
at narrow widths a bounded panel sits above it with its own settings scroll and
always reachable status/Apply/Cancel. Native PDF output remains the visual authority.
Standalone dialog callers keep the current modal mode. This cycle does not implement
arbitrary free-angle paragraph rotation, new producer/script admission or native
publication rules. Direct caret editing over original PDF glyphs remains a later gap.

## User workflow

Edit text -> select a run -> Edit paragraph continues to perform the existing
suggestion or nested-form promotion. It opens a nonmodal paragraph workspace on the
actual document page. The existing styled text input, typing Undo/Redo, range formats,
font picker, alignment, spacing, Fit area and Reset remain available; uniform fallback
remains supported. A visible area outline can be dragged or nudged to move it. Four
corners and four edge handles resize the area. Resizing reflows at unchanged fonts and
sizes, keeping the opposite edge/corner fixed. Native preservation/fit failures stay
visible; no hidden shrink-to-fit or bitmap editing fallback.

Keep the current explicit Preview paragraph button. Successful native output appears
on the main page with Original/Edited comparison. Completing an area gesture queues
a debounced native preview; typing and formatting may also queue preview after a
short idle period when valid, using the same candidate ownership/revision. No native
request per pointer move. A pending draft shows only its outline; it does not pretend
to paint resized PDF text. Apply is enabled only for the current completely rendered
candidate and valid active fields. All draft changes become one document Undo step.
The styled editor's local typing/formatting history remains separate.

Selection adjustment uses original-page text piece targets and the existing list.
The saved page crop clips pointer targets; hidden pieces remain inspectable in the
explicit source piece list. The list describes source pieces, not invisible targets.
Only selected paragraph controls receive editing gestures; other page mutations,
tool/tab switches, save and document Undo stay blocked during the session. Reading
zoom, scroll, page navigation/history and Locate paragraph stay available. Cancel or
Escape abandons the session; an active drag consumes Escape first. A font picker is
a true nested modal: it blocks page gestures/navigation while open, then restores
focus to the text editor. Apply/Cancel restore focus to Edit text.

## State and ownership

Keep the two existing paragraph state models and native transaction APIs. Add an
optional canvas presentation contract to each editor, not a second mutation engine.
The contract supplies target page/session identity, full source-axis bounds, current
source pieces/selection, selection mode, area mutation callbacks, busy/valid flags,
original/candidate raster state and render acknowledgement. A shared presentation
hook/shell owns original raster loading, comparison and responsive arrangement.
The controller continues to own candidate source, render URL and custom font leases.

Tie every candidate to the complete request revision. Invalid local numeric fields,
selection changes, new valid bounds, text/format changes and session changes invalidate
it; cursor-only selection changes retain it. Late replies are released. Do not
accidentally reset valid drafts when selected reading page or zoom changes. Keep the
target mounted when virtualized or away in single-page mode; distinguish live document
page/source mutation from selection-only reading changes. Source ownership checks
include every tab's retained Undo/Redo sources. One candidate request may run at a
time; coalesce later revisions and retain fonts until the pending request settles.

Zoom changes refresh presentation without publishing a different PDF. Preserve a
valid candidate's source during rerender, and block Apply until its newly requested
raster is available. Candidate URLs in source axes must not be passed through the
existing image workspace's inverse-native-rotation path. Explicitly tag paragraph
rasters as source-axis; PageView uses source width/height for the image and the normal
SVG page rotation/crop transform for both raster and text/area targets. Overlays are
already composited by renderParagraphPage and must not be painted twice. Prefer a
bounded width derived from source page width, zoom and DPR (600..2400), while modal
mode retains its current1600 request. Original/candidate URL release remains exact.

## Area controls

Bounds are source physical points. Use existing page pointer transforms for intrinsic
versus workspace rotation and page crop. Valid bounds stay within the complete page,
with minimum width/height0.01pt matching paragraph numeric fields; page crop affects
visible hit targets, not native bounds admission. Clamp move/resize without allowing
crossing the fixed opposite edge. Preserve pointer grab offset, primary pointer,
pointer capture and a3CSSpx activation threshold. Local gesture state does not mutate
the controller until completion; Escape/cancel/lost capture, session/source/zoom/page
rotation/crop changes, disabled state or unmount discard it.

Keyboard body arrows move1pt, Shift10pt, in screen directions. Handle arrows move
their edge/corner with the same convention. Edge handles constrain the appropriate
source axis after mapping the screen delta. Hidden page/crop-excluded handles are
not focusable. Handles have screen-sized24px targets; focus is visible and role/name
describe move/resize. Do not steal text caret focus on every rerender or preview.
Keyboard Escape consumed by font picker, text editor or active drag must not cancel
an underlying session accidentally. Prevent global document shortcuts from stealing
the editor's local Undo/Redo or selection.

## Acceptance

Frontend regressions: signed pointer mapping through all workspace rotations/page
crops; constrained edges/corners, move, grab offset, threshold, cancel/competing
pointers, keyboard/focus; uniform and mixed controller state; invalid hidden fields;
pending/latest preview and lease cleanup; nested picker; navigation/session identity;
source-axis raster alignment and no duplicated overlays; one Apply/Undo/Redo.

Fresh Windows build: edit a mixed paragraph in the real viewport, type/format, resize
and move with pointer/keyboard; compare native result; zoom/navigation/single-page;
Cancel/Escape and Undo/Redo; preview failure recovery; saved reopen/re-edit; narrow
controls and minimum window actions. Include a rotated/cropped/UserUnit authored
fixture with annotations/highlights and a duplicate occurrence; use the private quiz
locally where it proves the original user's insertion workflow, never in review scope.
Independently inspect saved text/styles, expected line wrapping and measured area,
outside-area pixels, sibling occurrences, page geometry, annotations and unchanged
input hashes. Native proofs remain authoritative; never weaken them for UI success.
Keep all failures and exact source/binary/PDFium hashes and stop only owned test apps.
