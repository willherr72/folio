# Folio v0.13.0 verification

Verified on Windows x64 on 2026-09-14 with bundled PDFium chromium/8044.
[Machine-readable results](verification-v0.13.0.json) identify the tested packaged
executable; the portable ZIP contains that exact executable.

## Automated checks

- Full native suite: 186 tests passed, zero failed; three existing opt-in tests ignored.
- Frontend: 290 tests across 52 files passed with two test workers. Two existing tests timed out under unrestricted concurrent load; both passed unchanged on focused rerun and in the complete bounded-worker run.
- TypeScript/Vite, Rust formatting and optimized Tauri production build: passed.
- Independent source review: no outstanding findings after resource-alias pruning, original-source admission and pre-allocation resource-budget fixes.
- [Five form reference pairs](issue15-form-reader-verification.json) pass exact MuPDF 1.27.1 text, glyph geometry and 2x raster, plus pypdf 6.14.2 text. Exports contain the edited first page and unchanged second page; references are independently authored.
- The explicit leaf Resources case positively copies changed text in both readers. Four inherited-leaf cases preserve pypdf’s existing empty extraction; equality there does not demonstrate successful pypdf copying.
- Native form coverage includes shared copies on the same and another page, nested forms, inherited leaf resources, positive dictionary/page transforms, page crop/rotation, stale identity, longer and shorter replacements, source immutability, collision/bounds, unsupported state and font encodings, and ordinary image-only pages.
- [Repeated-edit check](issue15-form-resource-verification.json): eight edits retain ten PDF objects each. Closed source IDs become unavailable. This is a fixture ownership/object-growth check, not a working-set benchmark.
- New form APIs bound graph expansion and direct resource cloning before native page copying/traversal. Cycles, excessive work, unsupported inline images and amplified resource dictionaries have regression coverage. This does not change ordinary opening or claim general renderer hardening.
- Locked third-party Rust/npm packages are unchanged from v0.12.0; the package retains 551 dependency records and license/provenance notices.

## Packaged Windows acceptance

An isolated production WebView used the packaged executable and real Rust/PDFium
backend with native Open and Save dialogs.

- Select the first nested occurrence, confirm its dialog hint, apply a longer replacement and retain the sibling. Undo and redo the single edit.
- Search finds the changed phrase.
- Save/reopen succeeds with the original moved, and the original hash stays unchanged.
- An additional unsaved edit survives a forced stop and recovery with both original and saved paths unavailable.
- Independent readers positively extract the saved practice replacement. Page 2 retains exact MuPDF text, glyph geometry and raster plus pypdf text.
- The occurrence modal and recovered page were visually inspected. Test helpers match the owned PID, executable path and native dialog. The owned test process was stopped afterward.

## Scope

Issue #15’s B3 stage supports editing one selected occurrence of conservative
text-only nested forms using standard Latin Type1 fonts, printable ASCII and
positive scale/translation. Each edit proves original/no-op page appearance,
selected identity and unchanged neighboring occurrences before publishing a new
immutable source. Published PDFs retain nested forms.

Embedded/custom fonts, grouped form text, font substitution, explicit spacing,
rotated/sheared form transforms, tagged/layered sources, source annotations,
clipping/transparency/images and uncertain inherited state are refused in this
stage. Existing top-level text editing and added annotations retain their prior
features. See [editing behavior](existing-text-editing.md) and
[issue status](issue15-status.md).

Physical printing, interactive Foxit/Acrobat acceptance, Windows IME acceptance
and 8 GB hardware acceptance are not claimed here. Outstanding #14 and #10 gates
remain separate.
