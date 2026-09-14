# Folio v0.12.0 verification

Verified on Windows x64 on 2026-09-14 with bundled PDFium chromium/8044.
[Machine-readable results](verification-v0.12.0.json) identify the tested packaged
executable; the portable ZIP contains that exact executable.

## Automated checks

- Full native suite: 160 tests passed, zero failed; three existing opt-in tests ignored.
- Frontend: 284 tests across 50 files passed. TypeScript/Vite, Rust formatting and the optimized Tauri production build passed.
- Independent source review found no outstanding blocker. Review identified PDFium's rotated reading-order difference; the group proof now uses private source-axis inspection while preserving and checking the original rendered orientation.
- [32 group reference cases](issue15-group-reader-verification.json) pass native reference glyph/raster/export checks, exact MuPDF 1.27.1 text/raster, and exact pypdf 6.14.2 text. References are separately authored single-run PDFs; the verifier does not normalize whitespace or loosen raster equality.
- Within-word splits are covered at all four cropped page rotations. Leading/trailing space splits pass at 0°/180° in these fixtures; 90°/270° space-boundary sources are four explicitly asserted appearance refusals. These are fixture outcomes, not a blanket rotation-based support rule.
- Additional group tests cover the eight-piece maximum, generated/doubled/omitted spaces, rows/gaps/styles/transforms/graphics, font aliases versus distinct dictionaries, tagged content, verified embedded font reuse and missing glyphs, stale selections, immutable sources and original-file protection. All three included practice groups pass.
- A private worker test repeats inspection ten times without adding a source. Stale/empty/unsupported-glyph/crop errors leave only the original registered source; success adds only the returned source, and closing both empties the registry. These are ownership checks, not a process working-set benchmark.
- Locked third-party Rust/npm packages are unchanged from v0.11.0; the package retains 551 audited dependency records and license/provenance notices.

## Packaged Windows acceptance

An isolated production WebView drove the native Open and Save dialogs against the
included practice PDF, using the packaged executable and real Rust/PDFium backend.

- Explicitly select the three Invoice pieces, check the exact combined text, apply a longer replacement, and undo/redo the single edit.
- Search finds the changed phrase.
- Save/reopen succeeds after moving the original; the original file hash stays unchanged.
- A further unsaved replacement survives a forced stop and workspace recovery with both the original and saved paths unavailable.
- The group modal and recovered page were visually inspected. Test helpers match the owned PID, executable path and native dialog before acting. The owned test process was stopped afterward.

## Scope

Issue #15 now supports explicitly selected, verified adjacent groups. Source text
must satisfy the existing unshaped Latin font proofs. Marked/tagged documents,
explicit spacing, nested forms, ambiguous character attribution and groups that
change even the validation raster remain refused. Group font substitution and
paragraph reflow are not included. Fractional glyph advances can make a visually
plausible group fail the exact appearance check.

See [editing behavior](existing-text-editing.md) and [issue status](issue15-status.md).
The existing B1 positioning matrix and older font/shaping regressions pass in the
full suite. Nested-form isolation remains open under #15. Physical printing,
Foxit/Acrobat interactive acceptance, Windows IME acceptance and 8 GB hardware
acceptance are not claimed here; the outstanding #14 and #10 gates remain separate.
