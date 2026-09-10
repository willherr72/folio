# Folio frontend verification

Date: 2026-09-10
Branch: `prototype`

## Implemented

- React and TypeScript desktop workspace served by Vite on strict port 1420.
- Native Tauri v2 adapter for `open_pdf`, `render_page`, `export_pdf`, `close_document`, and `engine_status` using the binding camelCase payload contract.
- Explicit browser demo only through `?demo=1` or **Explore demo**. Native errors remain visible and never activate demo content.
- Cached native page images in the thumbnail rail and selected-page viewport, bounded to 60 entries with Blob URL cleanup when a source closes.
- SVG page and overlay rendering in original PDF point coordinates with complete-page clockwise rotation and inverse pointer mapping.
- Immutable edit history for text, signature ink, movement, rotation, reorder, duplicate, delete, merge, undo, and redo.
- Helvetica/Arial text preview with baseline `y + fontSize` and multiline leading `1.2 * fontSize`, matching the native export contract.
- Signature drawing pad followed by page placement, direct overlay dragging, useful text/ink properties, zoom, keyboard shortcuts, unsaved-open/close guards, and Save As export.
- Demo export is clearly labeled and downloads `folio-demo-edit-plan.json`; it does not represent a PDF save.

## Verification evidence

### Unit and component tests

Command: `npm test`

Result: PASS. 3 test files, 12 tests, 0 failures.

Covered behaviors: immutable undo/redo and history branching; duplicate/reorder page identity; display dimensions, SVG transforms, forward mapping, and inverse pointer mapping at 0°, 90°, 180°, and 270°; explicit demo activation from the empty state; Ctrl+Shift+Z redo behavior.

### Typecheck and production bundle

Command: `npm run build`

Result: PASS. TypeScript completed with no diagnostics. Vite 6.4.3 transformed 1,589 modules and emitted the production bundle in 4.20 seconds. Main JavaScript was 181.15 kB (57.90 kB gzip); CSS was 12.70 kB (3.78 kB gzip).

### Browser interaction smoke test

Command: `node scripts/smoke-ui.mjs`

Result: PASS with no page console errors.

Validated nine workflows: demo open; text placement/editing; signature drawing/placement; rotation; page reorder; duplicate/delete; undo/redo (including Ctrl+Shift+Z); dragged overlay coordinates and exported edit-plan contents; saved-state indicator. Screenshots and structured results are in `artifacts/ui-smoke/`.

## Integration boundary

The browser demo uses deterministic generated SVG pages. It never intercepts the native path. Real PDF rendering and Save As behavior require the Tauri/PDFium backend and are verified by the native integration workstream.