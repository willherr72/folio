# Explicit paragraph fonts in automation

## Intent and evidence

Give CLI/MCP users the desktop's explicit font choice when embedded subsets lack
new characters. The original synthetic Word paragraph cannot add `this` using its
subset, but existing desktop paragraph substitution with installed Arial passes
Preview/Apply/Save and independent outside-region/second-page checks. Evidence:
`artifacts/automation-font-gap-20261028/desktop-paragraph-1`. Single-run context
substitution remains a separate gap. Do not weaken any native PDF proof.

The user authorized autonomous implementation and opted out of routine approval
questions. The following implementation choices are the coordinator's decisions.

## Approaches and choice

1. Automatically find a similar installed font: convenient, but machine-dependent
   and changes appearance without explicit intent. Reject.
2. Accept arbitrary font paths in MCP tool arguments: simple, but expands the
   existing launch-only filesystem authorization model. Reject.
3. Explicit font files and content IDs, reusing native attributed paragraphs:
   selected. CLI supplies fonts for an invocation; MCP supplies them at launch.
   It requires a small discovery step but makes font choice reproducible.

## Contract

- `folio-cli fonts --font FONT [--font FONT ...]` validates supplied standalone
  fonts and returns a compact catalog without loading PDFium. No installed-font
  scan, filesystem paths, raw bytes or large coverage arrays in the response.
- `preview-paragraph` accepts repeated `--font PATH`. Its JSON adds optional
  `fontOverrides:[{styleId,fontId}]`: at most64 unique source style IDs, each used
  by the requested runs. IDs are lowercase SHA256 of exact font bytes. All other
  formatting remains source-derived. Empty list means no override; null, unknown
  fields/IDs, duplicate styles, unselected styles and unauthorized fonts refuse.
- Font files: at most16 supplied paths, nonempty regular files, at most16MiB each
  and64MiB total bytes read (including duplicate inputs). Check limits before
  parsing; bounded reads catch growth. Parse with existing FontAsset restrictions.
  Deduplicate identical assets by content ID after charging input bytes. Catalog
  names at most512 UTF-8 bytes; reject oversized metadata rather than truncate.
- The catalog entries are `{fontId,name,weight,italic,bytes}`. Font IDs and byte
  sizes are verified from loaded immutable bytes. Use the same assets to register
  with the native engine; temporary registrations are rolled back on every exit
  without deleting pre-existing or later-claimed registry entries.
- Explicit overrides always disable the unchanged-byte shortcut, including
  unchanged text. Without overrides, existing exact no-op identity remains.
- Preview metadata retains the selected overrides and compact catalog of used
  fonts. Saved bundles contain the resulting embedded font programs, require no
  original font files at Save/Apply time, and retain manifest/candidate hash checks.
  Older bundles without font metadata remain readable. Retain existing32KiB edit,
  64KiB manifest and native glyph/grapheme/area/collision/protection constraints.
- MCP launch adds repeated `--font PATH` (optional). Capture validated font bytes
  in an immutable bounded FontInputs set; mutation/deletion of original files
  cannot affect later requests. `folio_list_fonts {}` returns authorized choices.
  Paragraph tool accepts only IDs from this set. Workers receive private temporary
  files for only referenced assets. Keep their handles owned until the worker is
  reaped; use existing Windows delete-on-close/Unix0600 snapshot machinery.
  No tool-time font path, font scanning, network or automatic fallback.

## Acceptance and limits

CLI and modern/legacy MCP must inspect, preview with a chosen font, Apply, inspect
again, edit again and save without original font files. Native public fixtures
cover page and nested paragraphs, source styles, unchanged-text explicit changes,
missing glyphs, stale selections, source/neighbor/other-page preservation and
protected-document permissions. Malformed, oversized, unknown, duplicate and
unauthorized input must fail before publication with originals intact.

Fresh binaries must exercise the original synthetic Word example with explicit
font bytes and independent saved text/pixel/graph checks. Existing no-op acceptance
must stay green. Document exact evidence and limitations. No new desktop UI,
single-run substitution, CFF/TTC support, silent font matching or parity claim.
Preserve originals, accepted package, prior evidence and the dirty worktree.
