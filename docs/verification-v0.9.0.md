# Folio v0.9.0 verification

Verified on Windows x64 on 2026-09-11, using the bundled PDFium chromium/8044. This release polishes font selection and implements bounded embedded-font editing (#12); #13 is a documented investigation with future implementation issues.

- **209 frontend tests passed in 43 files.** Includes preview/Apply semantics, keyboard behavior, temporary resource leases, canceled/late font acquisition, cross-dialog and Open/Add registration ordering, explicit substitution, retained drafts, source undo/redo and export.
- **92 native tests passed; three existing opt-in checks remain ignored.** Embedded tests cover real ReportLab simple subsets, WinAnsi and Identity-H CID TrueType fonts, precomposed accents, missing glyphs, all page rotations, original bytes and neighboring pixels, chained edits, self-contained substitution, export/reopen, standard-font substitution, marked-content refusal and unsupported Type3 inspection. Malformed/ambiguous Unicode, duplicate maps, ligatures, widths, conflicting and missing Unicode-subtable entries, oversized/shared font graphs and CMap work bounds have dedicated cases. Recovery restores and re-edits the substituted source after original PDF/font files and the temporary registry asset have disappeared.
- **12 real-browser scenarios passed.** Eight picker cases combine light/dark themes with 1280×820, 900×600, 900×420 and 390×740 viewports. Four composed substitution-dialog cases check the missing-glyph draft, nested chooser transition, preview, final apply and focus restoration at normal/minimum desktop sizes. Actual DejaVu font bytes load in the browser; no page errors occurred.
- **Production TypeScript/Vite/Tauri build, Rust formatting and diff checks passed.** Production dependencies are unchanged from v0.8.0. The package includes notices for 546 dependency packages.

## Packaged Windows workflow and independent readers

The isolated production app opened a synthetic ReportLab subset PDF, added a text note, replaced a run with the longer `Original sentence café` using its original subset, then correctly refused the absent è in `Original sentence crème` while preserving the draft. An explicit installed Arial preview was applied, followed by the text edit. Undo/redo, accented search, mouse selection and Ctrl+C returned the expected replacement. Editable and flattened copies reopened after the original PDF was moved. Added-note controls were restored only for the editable copy, and missing source Unicode mappings remained uneditable. No WebView page errors occurred.

Native dialogs were submitted to the owned test process. The Open split button did not expose UI Automation Invoke, so the harness used a process-verified Win32 button click. Save dialogs produced their default `Folio edited.pdf`; those newly generated files were moved to the distinct expected fixture names before reopening. These are automation accommodations, not application changes.

PyMuPDF 1.27.1 and pypdf 6.14.2 both extracted `Original sentence crème`, `Résumé café`, and `Keep this neighbor` from both exports. The substitute font program hash exactly matches the chosen preview font. Neighbor bounds were unchanged, and 200% MuPDF renders had **zero differing pixels outside the replaced run and explicitly added note**. Original source bytes remained unchanged. The independent render and production chooser were visually inspected. pypdf's editable-page extraction excludes the note annotation, as expected; flattened output includes it.

[Machine-readable results](verification-v0.9.0.json) include measurements, font/output hashes and fixture provenance. Tested and shipped executable SHA-256:

`a0350d8bdf6fb339443ea331348fb8c5dcb634a2b30865615056a089a488dfee`

Independent review verified fixes for native font/CMap resource bounds, conflicting nominal glyph maps, font acquisition races and substitution resource/operator isolation. No unresolved findings remain in the implemented scope.

## Complex-text investigation and limits

The [research probe](complex-text-design.md) shapes seven standalone samples plus a mixed-direction run sample. Its six-page cluster experiment deliberately retains failing extraction strategies: mapping a whole cluster to each glyph duplicates combining marks in all three readers; ActualText fixes PDFium and MuPDF for the probe but pypdf still duplicates marks. Adding ActualText preserves the paired MuPDF raster. These measured failures are an implementation gate, not a passing claim for complex-script PDF editing. Native setter/create/save integration, Arabic/Indic export, IME, broad script coverage, wrapping and form editing remain future work in #14–#16 under #13.

Existing-text editing remains horizontal Latin with positively verified mappings. No new physical 8 GB laptop, Foxit performance parity or real-printer claim is made.

## Reproduction

Run `npm test`, `cargo test --offline --locked --manifest-path src-tauri/Cargo.toml --tests --lib`, and `cargo fmt --manifest-path src-tauri/Cargo.toml --check`. Serve Vite on port 1432, then run `node scripts/smoke-font-picker-browser.mjs` and `node scripts/smoke-embedded-dialog-browser.mjs`.

For desktop reproduction, use `python scripts/generate-embedded-text-fixtures.py` in a fresh artifact directory, launch the production app with `scripts/corpus-start-desktop.ps1` on port 9247, then run `node scripts/smoke-embedded-text-desktop.mjs` and submit its announced native dialogs. It moves only generated test files and expects distinct editable/flattened outputs. Run `python -X utf8 scripts/inspect-embedded-text.py` with PyMuPDF and pypdf for independent inspection. Checked-in native fixtures regenerate separately with `scripts/generate-embedded-native-fixtures.py`; their generator versions, licenses and hashes are recorded alongside them.
