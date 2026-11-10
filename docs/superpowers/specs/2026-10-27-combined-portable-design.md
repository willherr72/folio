# Combined desktop and automation checkpoint

The user wants continued autonomous progress toward a useful everyday editor and
values CLI/MCP automation. The last form cycle is accepted. The standard portable
package currently omits folio-cli.exe and the README points to an old desktop
checkpoint. Consolidate the current product so the work is directly usable.

Coordinator choice: extend the existing versioned portable staging/publication
pipeline, rather than maintain separate desktop/CLI distributions. Both executables
use the same renderer and source fingerprint. A new development version
`0.22.0-dev.1` preserves the existing 0.21 package and launcher convention. This is
a local development checkpoint, not a published release. No new dependencies,
commits, cleanup, push or restart of the user's editor.

Build the desktop release with Tauri and explicitly build the CLI release, both
locked/offline, one Cargo at a time, jobs=1 and the established shared target.
Fingerprint the input before both builds and check afterward. Include Folio.exe,
folio-cli.exe, resources/pdfium, docs/automation.md, licenses, examples and existing
notices. Record both executable hashes and the renderer hash in build-info.json.
Existing staging, destination refusal and ZIP checksum behavior stay in force.

The verifier must run on an explicitly supplied extracted package and archive as
well as the default current version. It must require both executables and the
guide, validate source-manifest internal fingerprint and build-info agreement,
hash both executables and PDFium, compare every ZIP entry to the package without
extracting arbitrary entries, and retain OCR/license checks. Expose a callable
verify_package(root, archive, expected_version=None) for real fixture tests. A
report path is optional; reports are created exclusively, never overwrite evidence.
This is accidental corruption/integrity checking, not publisher authentication.

Acceptance: negative controls for absent/changed CLI, changed source manifest,
duplicate/different archive entries and existing report; fresh frontend suite and
release build; current native regression gate; extracted-package text/paragraph/form
automation with independent saved outputs; desktop launch/read smoke in a separate
owned process if the existing desktop workflow is available. No whole-app GUI,
printing, 8 GB hardware, broad producer, accessibility or commercial-parity claim.

README must identify the new local checkpoint, explain desktop and automation entry
points and link limits. Historical checkpoint evidence stays labeled historical.
One independent final scoped review, then demonstrated Important/Critical fixes
with RED/GREEN controls. Overall goal remains active.
