# Folio v0.11.0 verification

Verified on Windows x64 on 2026-09-13 with bundled PDFium chromium/8044.
[Machine-readable results](verification-v0.11.0.json) identify the tested packaged
executable; the portable ZIP contains that exact executable.

## Automated checks

- Full native suite: 150 tests passed, zero failed; three existing opt-in tests ignored.
- One additional repeated-edit resource regression passed after the full suite.
- Frontend: 276 tests across 49 files passed. TypeScript/Vite, Rust formatting and the optimized Tauri production build passed.
- Independent source review found no outstanding blocker. The review identified dormant spacing loss; a failing reference-geometry regression reproduced it before explicit-state routing fixed it.
- [144 reference-PDF cases](issue15-reader-verification.json) cover nine states, four page rotations and four replacement lengths. Native glyph/raster/export-reopen comparisons pass; MuPDF 1.27.1 returns exact reference text and raster, and pypdf 6.14.2 returns exact reference text.
- Native tests additionally cover multibyte embedded fonts, leading/trailing/consecutive gaps, dormant spacing, shorthand operators, split streams, relative-neighbor refusals, ambiguous mappings, deep graphics state and immutable source protection.
- [Twenty repeated edits](issue15-resource-verification.json) retain seven PDF objects; output size stabilizes at 947 bytes. Closed source IDs become unavailable and the original file stays unchanged. This is not a process-memory or lower-memory hardware benchmark.
- Locked third-party Rust/npm dependencies are unchanged from v0.10.1. The package retains its 551 audited dependency records and full license/provenance notices.

## Packaged Windows acceptance

An isolated production WebView drove native Open and Save dialogs using the
included positioned-text practice PDF. No mocked editing backend or test IPC
replaced the native implementation.

- Rotated, skewed/scaled, character/word-spaced and explicitly positioned samples accept longer replacements; each passes undo/redo.
- The edited positioned phrase is searchable.
- Native save/reopen succeeds with the original moved; the original file hash is unchanged.
- After a forced stop of the owned test app, recovery restores an unsaved spaced-text replacement with both original and saved file paths unavailable.
- The recovered page was visually inspected. A bounded helper matched the owned process and exact native dialog title/control before accepting paths.

The first attempt to start the smoke script preceded completion of the asynchronous
app launch; its log directory did not yet exist. Waiting for the launch record and
running the script completed successfully. No application change was needed.

## Scope

This release implements bounded same-font single-run positioning, with deliberate
refusals for ambiguous mappings, relative-neighbor movement and font substitution
on explicitly spaced pages. Issue #15 stays open for selected adjacent groups and
isolated nested-form occurrences. [Behavior and limits](existing-text-editing.md)
and [issue status](issue15-status.md) describe the contract.

No physical printing, Foxit/Acrobat interactive acceptance, Windows IME acceptance,
or 8 GB hardware result is claimed by these tests. The outstanding #14 and #10
gates remain separate.
