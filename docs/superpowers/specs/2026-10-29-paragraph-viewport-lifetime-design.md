# Preserve the document viewport across paragraph sessions

The user authorized autonomous implementation and opted out of routine questions.
This is a coordinator-selected lifecycle correction, not a proved memory fix.

## Evidence and intent

The source raster correction removes duplicate original rendering, but the actual
optimized Word and mixed workflows still exceed the unchanged 128 MiB tree-growth
budget. Opening and cancelling the workspace without any candidate also fails.
Hiding the workspace page pane reduces that growth; shadow/background controls do
not resolve it. App currently replaces DocumentViewport with a dialog-owned
DocumentViewport and replaces it again on Cancel. Preserve that viewport's mounted
identity, scroll state and page resources while changing the editing controls.

## Approaches

Keeping every old editor mounted would retain drafts and sources unnecessarily.
Moving the viewport with a portal would still change its React parent/container.
Choose one stable viewport host with a sibling paragraph panel. The dialog keeps
its existing state, preview ownership and validation, and publishes only the
current canvas presentation to the host. Image editing remains on its existing
route; this is not a general workspace rewrite.

## Interface and ownership

Add ParagraphViewportHost, scoped to the active document tab. It always renders
one keyed viewport slot in the same tree position. A matching paragraph workspace
publishes its CanvasParagraphView in a layout effect through a stable context.
Publication is bound to a session key and page id; owner-specific cleanup cannot
clear a newer publication. A mismatched or closed session cannot expose an old
preview. Before the first matching publication, the host supplies a disabled,
loading presentation for the selected paragraph page. After Cancel/Apply it uses
the ordinary document presentation immediately, ignoring stored old views.

ParagraphWorkspace retains its original rendering and preview retain/release
logic. In a matching host it renders the panel and picker only; outside a host it
retains its existing self-contained layout. Parent updates must not cause a
publish/render feedback loop. No state setters during render.

## Acceptance

An App regression must fail on current viewport DOM replacement, then prove the
same node survives entering the editor, Preview, Cancel, reopening, Apply and
Undo/Redo. Host tests cover stale session/page publications, owner cleanup and
loading/read-only initialization. Existing canvas, font picker, zoom, reading
navigation, source cache and candidate lifetime tests must pass.

Run the full frontend suite and production build, then a fresh optimized local
candidate bound to source. Repeat original Word/mixed 23-cycle workloads with
unchanged assertions/budgets and the narrow saved-output independent audit. A DOM
identity pass is not a memory-budget pass. Preserve all failed evidence and prior
packages. No forced GC, blocked diagnostics, native/renderer changes, commits or
publication. One final portable milestone review remains required.

## Runtime refinement: raster element lifetime

The optimized host candidate passes functional Word/mixed/narrow workflows but still fails the original resource-growth budget. A WeakRef-only observer confirms the viewport and SVG persist across all sessions, while the image element is replaced during loading. Removing the observer extra bitmap decode does not fix growth. PageView conditionally exchanges image and white rectangle whenever its raster URL is temporarily absent.

Extend the same lifetime correction to one stable keyed image element in PageView. Keep the white placeholder during loading and remove the image href while no current raster exists; never retain old visible content as a shortcut. This changes mounted React structure, not detached-element attributes or any blocked diagnostic. Native rendering, URL ownership, annotations, transforms and interaction admission stay unchanged. Do not change thumbnail structure. Add a failing DOM identity test through loading, error, ready and return-to-reading transitions, then run rotation/crop/canvas/source-lifetime regression and fresh optimized acceptance. Stable image identity is still not a memory-fix claim.
