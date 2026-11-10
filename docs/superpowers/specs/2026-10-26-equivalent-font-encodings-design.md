# Equivalent font encoding dictionaries

Allow existing text editing when a PDF expresses an already supported encoding
as an explicitly equivalent dictionary. The current `text_edit` preflight rejects
even `/BaseEncoding /WinAnsiEncoding /Differences [65 /A]`. Embedded simple-font
verification likewise accepts only the name object. This blocks safe edits based
on representation rather than a difference in character meaning.

The user authorized autonomous continuation. The coordinator chooses this small
compatibility correction; no new UI or font substitution policy is required.

## Approach

Use one bounded, read-only helper to recognize the original named encoding or an
equivalent dictionary. Compared with removing the guard, this establishes exact
glyph-name identity before admission. Compared with arbitrary custom-encoding
support, it leaves source mapping and font metrics unchanged.

Named StandardEncoding, WinAnsiEncoding and MacRomanEncoding retain their existing
policy. Dictionaries require explicit WinAnsiEncoding or MacRomanEncoding, valid
optional Type, and absent/empty Differences or exact canonical ASCII glyph-name
assignments (codes32–126). No default base inference, aliases, alternate glyph
names, non-ASCII Differences or custom remapping. Bound arrays to512 entries,
reject duplicate assignments, dangling indices, out-of-range indices and malformed
types. Resolve indirect values through the existing bounded PDF resolver.

Integrate with page text preflight and embedded simple-font WinAnsi recognition.
Do not mutate source dictionaries or bypass existing cmap/width/font/no-op/raster,
neighbor, permission, occurrence, resource and paragraph checks. All original files
remain unchanged. Non-Latin/complex text capabilities do not change.

## Acceptance

Public engine tests must demonstrate the original failure and corrected same-font
edits, paragraph edits, explicit substitution, save/reopen, indirect encodings and
all quarter turns. Embedded fonts with and without ToUnicode still require matching
font cmap/widths. Negative tests include same-Unicode but different glyph names,
malformed arrays, indirect loops and actual remapping. Independent readers check
saved text/geometry and unaffected pixels; a fresh desktop check covers Preview,
Apply, Undo/Redo and save/reopen. One scoped fresh review, then inline corrections.

Specification reference: ISO32000-1 Table114 and AnnexD,
https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf.
This is a deliberately narrower subset than general encoding dictionaries. Synthetic
fixtures prove the representation correction; they do not establish broad producer
compatibility or Adobe/Foxit parity.
