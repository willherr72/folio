# Folio v0.4.0: recovery, search, and printing

The user approved issues #1, #2, #3 for this release. Subsequent milestones are v0.5 (#4,#5), v0.6 (#6,#7, extensive testing), and v0.7 (#8). GitHub Issues remains the central backlog.

## Recovery
Persist version-1 checkpoints in native app-local data, with immutable source PDF copies and two committed generations. Atomic manifest replacement preserves a previous usable checkpoint after interrupted writes. A lifetime filesystem lock prevents two processes overwriting the same workspace; a secondary process may edit with a visible recovery-unavailable warning. Store only native tabs and current editable document state, dirty flag, saved digest, active tab, zoom and scroll. Undo history is not restored. Source IDs are remapped when reopening snapshots. Originals are untouched and may move without breaking recovery.

On startup inspect recovery before allowing opens or autosaves. Offer Restore workspace / Discard recovery. Do not overwrite pending recovery. Corruption/missing recovery source errors remain visible and allow explicit discard. Debounce changes for one second, with a five-second maximum interval during continuous activity; serialize writes. Tab close checkpoints remaining tabs before releasing source handles. Confirmed normal window close drains writes and clears recovery; cancelled close retains it. Recovery is crash protection, separate from editable PDF persistence in #7.

## Search
Ctrl+F and a toolbar action open search for the active tab. Preserve query, selected match, and visibility independently per tab. Case-insensitive literal search normalizes whitespace and maps results to native glyph bounds. Scan asynchronously with stale-query cancellation and bounded caching. Display counts, previous/next, Enter/Shift+Enter and Escape. Highlight all matches and distinguish current match; navigate its rectangle into view at any page rotation/zoom. Explain scanned pages without text and extraction errors. No OCR.

## Printing
A Folio modal selects all/current/custom ranges, fit/actual sizing, and initial portrait/landscape orientation. Continue opens Windows printer selection/properties for printer/paper/copies. Export current edited page plans to a private temporary PDF, then rasterize and spool sequentially through GDI using actual printer DPI and printable margins. Keep bounded image memory and clean temporary resources on cancel/error. Printing does not mark edits saved or alter source files. Other platforms give a clear unavailable message. Native validation uses Microsoft Print to PDF only; do not send jobs to a physical printer automatically.

## Boundaries and validation
Windows x64, Tauri 2, Rust, React/TypeScript; no cloud or AI. Keep tab caching and batched text layout intact. No signatures library/highlights/editable saved PDFs/original-word editing in this release. Test-first checks cover recovery lifecycle/data integrity, source remapping, search geometry/stale results, print range/geometry. Run existing frontend/native suites, production build, native crash/restart, virtual-printer output inspection, browser keyboard/rotation selection and tab performance. Publish only after checks and independent code review.
