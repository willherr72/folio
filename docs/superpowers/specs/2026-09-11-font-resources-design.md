# Installed and imported font resources

Implement issue #11 for added text boxes. Keep standard fonts and add locally installed/file-imported static TrueType-outline fonts (TTF and compatible OTF containers). Reject unsupported collections, CFF, variable/color fonts, restricted embedding and missing glyphs with specific explanations. Complex shaping and existing embedded-text replacement remain #12/#13.

## Resource model

TextOverlay gains optional `fontId`; `fontName` retains its standard-font default for compatibility. A validated custom font takes precedence. Font IDs are SHA-256 of the immutable bytes. A native registry deduplicates assets, caps individual bytes at 16 MiB, total retained bytes at 128 MiB and count at 64. Font metadata describes display name, style and actual supported codepoint ranges. Catalog entries use opaque IDs bound to discovered local paths; selecting rereads and validates the file. Loading does not retain arbitrary installed fonts until selected.

The frontend loads the exact bytes into a private FontFace family before displaying custom-font text. Disable kerning/ligature substitutions to match the unshaped PDF text. Keep font resources referenced by all tabs' present/past/future states; release unused assets and FontFace objects after tab/history ownership ends. Failed or canceled choices release temporary assets. No font bytes in ordinary page plans or undo history.

## PDF persistence

Embed the complete TrueType program once per unique font in each exported PDF using a Type0/CIDFontType2 resource, widths, CID-to-glyph mapping and ToUnicode mapping. Both editable annotations and flattened output use the same generated text appearance. Retain font IDs in Folio annotation metadata, verify embedded bytes and appearance resources before importing editable annotations, and register the validated font on reopen. A source-font file is never needed to reopen an exported PDF.

Recovery stores content-addressed font snapshots separately from the JSON manifest. Validate byte lengths/checksums and paths, reuse unchanged files on routine checkpoints, retain resources for the last two valid generations, and restore fonts before restoring plans. Old manifests without custom fonts continue to work. Cleanup must remain confined to the owned recovery directories.

## User flow and validation

Properties keeps the twelve built-in faces and adds a searchable installed-font dialog with Import font. Unsupported entries explain why they cannot be chosen. Font selection validates current text before committing; text changes show unsupported characters and export/recovery never silently substitute glyphs. Initial custom-font text scope is unshaped left-to-right BMP Latin/extended Latin, Greek, Cyrillic and common punctuation/symbols when covered by the font; unsupported shaping-dependent text is explicit.

Test font-file removal, all edit/save/reopen/flatten/recovery paths, shared-font deduplication and cleanup, malformed/restricted fonts, missing characters, styles/rotations, preview readiness and asynchronous stale results. Check exported text/resources/rendering with independent readers. Ship after independent review, passing regression suites and a packaged desktop test.
