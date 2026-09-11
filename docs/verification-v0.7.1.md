# Folio v0.7.1 verification

Verified on Windows on 2026-09-11. This update allows existing-text replacements wider than their original runs and adds twelve standard font choices for added text boxes. [Supported scope](existing-text-editing.md) and [broader font roadmap](font-support-roadmap.md).

- **176 frontend tests passed in 38 files.** The new controls preserve content and positioning, apply font family/style/weight, retain the legacy Helvetica default, and preserve font choice in undo/redo and export plans.
- **63 native tests passed; three existing opt-in checks remain ignored.** The full final suite includes longer replacement success across four cropped page rotations, page-edge and new/increased text/graphics overlap rejection, original-source protection, compound-path and patterned-background regressions, all twelve font faces through editable export/reopen/recovery/flattening, face-dependent annotation bounds, and legacy/tampered font metadata handling.
- **48 real-browser font/rotation checks passed.** Each font was selected through Properties and compared against the rendered SVG font attributes and thumbnail. Selection extents followed actual text advances, including narrow/wide letters and common ligature sequences. No browser errors.
- **TypeScript/Vite/Tauri release build, Rust formatting and diff checks passed.** Runtime dependency versions are unchanged. Adapted ReportLab metric data has its source attribution and full BSD notice included in the distribution.

## Packaged desktop check

The complete production-app harness passed without resuming midway, using an isolated profile. Native file dialogs were submitted separately in that owned process. It verified:

1. Adding a Courier bold italic text box while keeping immediate text editing.
2. Rejecting an off-page replacement while retaining the typed input.
3. Replacing “Original sentence” with “New sentence with more detail”, preserving the annotation, undo/redo, search invalidation and actual mouse selection/Ctrl+C.
4. Saving without changing the original file's SHA-256, then reopening after moving the original out of its former path.
5. Retaining the wider replacement as editable source text and the annotation's Courier bold italic family/style; continuing to explain unsupported embedded fonts.

Independent pypdf inspection found the replacement and unchanged neighboring text, and confirmed `/Courier-BoldOblique` in the annotation appearance's font resources. PyMuPDF measured the original run's right edge at 174.50 points and the replacement at 267.86 points. With annotations excluded, 1,683 pixels changed, all inside the old/new text bounds plus a four-point inspection allowance. Visual inspection of the independently rendered saved PDF confirmed the annotation and replacement.

[Machine-readable results](verification-v0.7.1.json) preserve the output and original hashes. Tested and shipped executable SHA-256:

`eb438536f7401aafc9842c332c1d75fc68cc0508cf2bd4da940b4e9a4e24ad50`

Independent review found that page-covering bounds did not establish a uniform background. The final implementation proves a single opaque, unstroked rectangular page fill before exempting it, leaves images/compound paths subject to collision checks, and rejects named pattern paints. Both demonstrated regressions passed; no unresolved review findings remain within the documented source-page-object collision scope.

## Reproduction and limits

Run `npm test -- --run`, and from `src-tauri` run `cargo test --tests --lib --offline --locked`. Serve Vite on port 1431 and run `node scripts/smoke-text-fonts-browser.mjs` for the browser fixture. Generate owned PDFs with `python scripts/generate-text-edit-fixtures.py`, start the release executable using `scripts/corpus-start-desktop.ps1`, set `FOLIO_APP_PID`, `FOLIO_CDP_URL` and optionally `FOLIO_DIALOG_MANUAL=1`, then run `node scripts/smoke-text-improvements-desktop.mjs`. Finally run `python scripts/verify-text-improvements.py` with PyMuPDF and pypdf installed. Preserve the generated outputs; the desktop harness refuses to overwrite an existing output and deliberately moves its generated original.

No new performance claim is made. Embedded/subset fonts, installed/imported fonts, complex shaping, paragraph reflow and OCR remain future work. Collision checks are conservative and cover source page objects; separately placed annotations/overlays still need visual checking. Physical low-memory validation and opt-in real-printer tests remain separate work.
