# Existing-text removal with verified embedded TrueType fonts

## Intent and authority

Folio's visible text-removal workflow is verified for standard ASCII fonts, but
ordinary producer PDFs embed fonts. Extend the actual removal path to verified
simple TrueType and horizontal Identity-H/CIDFontType2 resources, including Latin
Unicode text and multibyte character codes. Keep retained glyphs in place and
leave original files intact. The user authorized autonomous development and
opted out of routine design questions; these implementation choices are ours.

Previous goal turn was progress: corrected two workflow defects and completed a
fresh build, 1367 frontend tests and desktop/independent-output acceptance.

## Approaches

1. Convert text to standard fonts: easy to remove, but changes typography and is
   incompatible with source fidelity. Rejected.
2. Trust extraction and delete native text objects: loses exact operand ownership
   and may move following text or import unrelated object graphs. Rejected.
3. Extend source operand accounting using existing embedded-font proofs and copy
   only the validated font resource graph into the fresh output. Chosen: preserves
   original code bytes, font program, widths and positions, with explicit bounds.

## Architecture

Add a private redaction font module. A document-scoped registry owns standard-font
metrics or an Arc of a verified embedded font, normalized bounded resource graph,
code width (1 or 2 bytes) and code-to-(Unicode,width) table. Reuse the existing
embedded_font verifier through a narrow engine bridge; do not create a weaker
second font verifier. The bridge verifies Latin editing repertoire and returns
exact PDF widths, not rounded physical advances. Every source operand must decode
completely with a unique verified mapping; excluded or incomplete codes refuse.

Normalize the embedded font graph through role-specific allowlists: root simple
TrueType or Type0, one CIDFontType2 descendant, descriptor, CIDSystemInfo, widths,
encoding and FontFile2/ToUnicode/CIDToGIDMap streams. Resolve references with cycle
and depth checks, reject unknown rendering fields, external streams and malformed
filters. Only raw or FlateDecode streams with null/absent DecodeParms are admitted.
Do not carry unrelated catalog, page, annotation, attachment or arbitrary stream
references. Font program metadata/maps remain part of the retained font resource;
this is text removal, not certified secure area redaction.

Bounds: at most 64 distinct used embedded resources per document; at most 16 MiB
per font program, 2 MiB per map, 64 MiB aggregate normalized font payload, 100000
font graph nodes and 64 levels. Charge before cloning/decompression growth. Cache
by actual source resource identity, retain Arc values across pages and deduplicate
publication by normalized resource digest. Repeated aliases/pages must not multiply
font-program copies. Existing input/content/output and session budgets still apply.

Content canonicalization tracks the active font through q/Q and asks the registry
to decode strings. Budget original bytes and decoded Unicode, keep canonical
operand bytes unchanged. Block/range offsets become Unicode scalar/glyph ordinals,
not UTF-8 offsets or encoded-byte offsets. Selection merging recognizes logical
U+0020 spaces; PDF word spacing applies only to single-byte code 32, never two-byte
0032/0020 or another code that happens to map to space. Glyph identity includes
original code and Unicode. Compensation uses original PDF width and spacing.

Fresh output publishes only owned normalized font resources and retained content.
Source selection digest includes normalized font resources/mapping, so altered
widths, programs, ToUnicode or codes cannot reuse stale selections. Re-open the
result and compare retained code, Unicode, origin/end within existing 0.002pt.
Native preview's unique source/native glyph mapping and fidelity checks continue
to guard Apply. No changes to approval, source ownership, history or Save routing.

## Acceptance

- Simple WinAnsi and two-byte Identity-H fixtures remove accented Latin and ASCII
  in a mixed-font page; untouched following text, TJ adjustments, Tc/Tw/Tz and q/Q
  retain coordinates and pixels. Multibyte ordinal handling must not panic.
- UTF-8 text before a selection, code32/space distinctions, partial source maps,
  malformed/incomplete codes, stale font changes and ambiguous mappings covered.
- Shared font across many pages/aliases remains one owned/published resource;
  graph cycles, unknown fields, oversized/invalid streams and aggregate exhaustion
  refuse before output. No unrelated graph imported.
- End-to-end preview/Apply, saved removal, re-open/re-edit with embedded font; a
  retained original source remains unchanged. Actual desktop mark/preview/history
  and independent PDFium/MuPDF text/geometry/raster checks on generated specimens.
- Focused native tests then full library, TypeScript/Vite, stable source build,
  independent final review with one coordinator correction pass. Preserve failures.

## Limits and builder discretion

Keep existing page/operator/annotation/graphics admission. CFF, vertical writing,
unproved ligatures, arbitrary CMaps and unsupported scripts remain future work;
do not advertise arbitrary PDF or secure-area support. Existing standard-font
behavior must remain unchanged. Builder chooses internal normalized-graph type,
test fixture helper layout and cache plumbing within the explicit budgets.
Keep inherited dirty work and artifact history; no commit, push, release, user-app
restart or original-file overwrite. Inline implementation with one design check
and one final review; overall goal remains active.
