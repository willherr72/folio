# Text-area editing and persistence checkpoint

Development milestone, October 22, 2026. The desktop editor now creates and resizes text areas, and editable PDF saves and recovery preserve them. This does not close #16 or change the downloadable v0.13.0 release.

## Editor controls

Choose **Text area**, drag a box on a page, choose a font and type in **Content**. The properties panel also offers **Add text area** for keyboard creation. Font selection must succeed before the empty area enters document history; cancelling leaves the document unchanged.

Select an area to move it or resize its four corners. Focus a corner and use arrow keys for one-point steps, or Shift for ten-point steps. Page and text rotation are respected, and resizing keeps the opposite corner fixed. New areas and resized corners stay within page bounds with a minimum 36-point side. Body movement, pointer resizing and keyboard resizing cannot compete. Escape or lost pointer capture discards an unfinished resize. A completed drag creates one undo entry.

Properties offer width, height, alignment, inset and line spacing. Numeric drafts apply on Enter or blur; Escape restores the previous value. Wrapping changes without changing the original text or font size. If text overflows, its source and selection frame remain editable so the area can be enlarged. Save refuses overflowing text. The tool uses the native custom-font Latin/LTR layout scope and is unavailable in the browser demo.

## Saved data

A text overlay can contain optional `area` settings: version 1, width, height, inset, line spacing and left/center/right alignment. Its existing text, font size, custom font ID, shaping options, color and rotation remain authoritative. Areas require an embedded custom font and explicit LTR shaping.

Folio retains original logical text, including repeated spaces, NBSP, CRLF/LF breaks, blank lines and trailing spaces. Reopening regenerates wrapping from that text and the exact font. Prepared character data retains unpainted delimiters. External readers continue to follow the [reader-compatible copying policy](text-area-copy-policy.md).

Editable area annotations use portable record version 4. Import verifies the embedded font, regenerated appearance/resources, bounds and annotation fields before restoring editable controls. Modified, unsupported or downgraded records remain native PDF annotations. Legacy single-line records retain their existing versions and rendering.

## Drafts and previews

Recovery snapshots retain area settings, original draft text and font bytes even when the draft overflows or contains unsupported characters. Invalid area structure is rejected. Export and native preview refuse overflow or unsupported layout instead of clipping or replacing text. Recovery therefore remains usable while an unfinished draft needs correction.

Restored areas use the native multiline preview. Cache keys include every area setting, duplicated pages copy settings independently, and the font picker validates candidates against the saved area. Properties keep custom-font LTR shaping enabled while allowing text, size, color and ligature edits.

Textarea edits map back to original source offsets so changing a letter preserves untouched CRLF/LF breaks elsewhere. Composition commits use the same mapping. New breaks inherit the surrounding paragraph's delimiter convention.

The native Latin layout's existing limits remain in force. Composition additionally bounds content to 8 MiB, estimated preview data to 8 MiB and semantic font resources to 16 MiB. Original font bytes use the existing font resource limits.

## Verification

Automated tests cover exact schema/source preservation, empty areas, overflow refusal, metadata tampering/downgrades, missing requirements and recovery after source/font removal. Sixteen page/text rotation combinations on a cropped, intrinsically rotated source preserve rendered output byte-for-byte after editable reopen and re-export. A separate raster comparison checks area composition against independently positioned native lines.

Frontend tests cover area cache isolation, queued snapshots, duplicate-page settings, editing safeguards and font selection with original CRLF text. Independent code review found textarea normalization of untouched CRLF breaks; insertion/deletion and composition regressions now cover the correction.

Verification commands (Windows, bundled PDFium and fixture fonts):

```powershell
npm.cmd test -- --maxWorkers=2 --minWorkers=2
npm.cmd run build
cargo test --manifest-path src-tauri/Cargo.toml -j 1
cargo check --manifest-path src-tauri/Cargo.toml --lib --no-default-features -j 1
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
python -B -m unittest discover -s scripts -p 'test*.py'
```

The persistence checkpoint passed 305 frontend tests across 54 files and 206 native tests, with three existing opt-in tests ignored, plus production build, library check without default features, formatting and 31 Python diagnostic tests.

The controls milestone passes 338 frontend tests across 57 files, the production typecheck/build and Rust formatting. Coverage includes geometry, rotated gestures, atomic font selection, cancellation, undo/redo, editable dimensions and competing-touch regressions. Focused native layout/overlay/persistence checks pass all 20 tests. Independent review found premature numeric clamping and competing touch gestures; both are fixed and covered by regressions. The browser smoke uses the real native preparer, confirms drag creation/resizing and rewrapping (nine rows at 180 points, five at 280 points), and retains source text during overflow. File dialogs and export are captured by the fixture; actual PDF round trips are covered by the native tests.

To repeat the browser smoke, build `cargo build --manifest-path src-tauri/Cargo.toml --example text-area-ui-probe -j 1`, run `python -B scripts/serve-text-area-ui.py --probe <path-to-built-executable>`, and start `npm.cmd run dev -- --port 1422`. Open `http://127.0.0.1:1422/tests/fixtures/text-area-controls.html` (use this exact host for the loopback bridge). The fixture offers DejaVu Sans and an empty page. Its save button records `window.savedAreaPlan` for assertions. Stop both local servers after testing.

Related: [design](superpowers/specs/2026-10-22-text-area-persistence-design.md), [implementation plan](superpowers/plans/2026-10-22-text-area-persistence.md).
