# Document-wide Find options

Folio's document search currently folds every query to lowercase, without case
or whole-word controls. Query and glyph folding disagree on contextual Greek
sigma (`ΟΣ` becomes `ος` as a query and `οσ` as glyphs). Improve ordinary Find
across existing text, added text and protected documents without changing PDFs.

## Approach and ownership

Extend the existing search pipeline and its location-only native transport.
Use one pinned Unicode 17 data table in TypeScript and Rust. Independent browser
and operating-system character policies can disagree; routing every search
through native extraction would lose the existing demo/overlay/cache paths.
The shared-table approach preserves those paths and gives deterministic matching.
This is the implementer's choice under the user's autonomous execution request.

No new runtime dependency, renderer change, portable version/promotion, clipboard
operation or private input is needed. Preserve the completed052 package and
selected050/separate051 checkpoints. This managed dirty worktree stays in place.

## Matching contract

- Literal search, never regex. Defaults remain case-insensitive, substring Find.
- Match case uses exact source scalar case. Otherwise use Unicode17 default
  full case folding (C+F, excluding Turkic T mappings) on query and source alike.
  Thus sigma variants agree and `STRASSE` matches `Straße`.
- Never match only part of an expansion created by folding: `s` does not match
  half of `ß`; rejected candidates cannot hide a later valid overlapping start.
  Preserve mapping to original glyphs, including multi-scalar physical glyphs.
- Retain existing ECMAScript whitespace collapse/trim, including BOM/NBSP and
  excluding NEL. No accent removal, canonical normalization or dictionary search.
- Whole words requires both ends to be word characters with no adjacent word
  character. Word characters are Unicode17 L/M/N/Pc plus ZWNJ/ZWJ. ASCII and
  curly apostrophes join a word only between word characters. Hyphens separate
  words; surrounding quotation marks do not join them. Punctuation-only queries
  have no whole-word results. Contiguous CJK letters form one token; no dictionary
  segmentation is implied. Phrases can contain punctuation/whitespace internally.
- Use the native existing1,024UTF-8-byte raw-query limit consistently; display an
  actionable error instead of silently truncating or dispatching an invalid query.
  Empty/whitespace UI queries remain the normal idle state. Reject lone surrogates.
- Keep protected search's250,000-character/1,000-match/10,000-rectangle limits,
  page units, rotation/crop behavior and text-free result shape. Search never
  exposes protected extracted text or bypasses prepared-print document policy.

## API and UI

Add `PageSearchOptions {matchCase,wholeWords}` with false defaults. Preserve the
public Rust `search_page(source,page,query)` method and add an options method;
the worker and optional Tauri argument carry the options. The TypeScript adapter
accepts an optional fourth argument. Existing three-argument callers still work.

Document search presents two labeled checkbox controls. Query, options and result
index belong to the active tab. Query/option edits reset to the first match,
immediately hide stale results and invalidate old asynchronous work. Closing or
switching tabs preserves the existing query/options lifecycle without mixing
results or changing document undo history. Enter/Shift+Enter and F3/Shift+F3 wrap;
Escape closes Find. Composition does not trigger shortcuts. Controls wrap within
760px and remain keyboard accessible.
Repeated Ctrl+F focuses/selects the existing query without resetting flags/index;
Ctrl+Tab keeps the application's tab navigation while focus is in Find. Retain
Find's ownership of other input keys so Delete cannot edit a selected PDF object.

## Validation and delivery

Use shared authored fixtures for case variants, fold expansions, combining marks,
supplementary characters, words/apostrophes, CJK tokens, whitespace and overlapping
rejected candidates. Exercise every retained fold entry in both implementations.
Verify generated tables against pinned original data and keep the Unicode license.
Test hook cancellation, tab isolation, overlays, restricted transport and defaults.

Run the complete frontend suite at two workers/default5000ms and TypeScript/Vite;
complete library correctness plus affected ordinary optimized native search,
text-geometry and OCR targets, with default/no-default feature checks. Build a
normal optimized GUI and verify actual owned Windows search on authored public
PDFs, including protection, options, keyboard wrap, zoom/rotation, tab switches
and narrow layout. Bind code, data, commands, binary and runtime evidence.

One fresh final feature review follows inline implementation; one consequential
fix pass, no duplicate review. Broader scripts/producers/readers, physical input,
performance peaks, printing, accessibility and parity remain separate acceptance.
