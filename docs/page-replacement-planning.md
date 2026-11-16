# Replacement planning and automation

Development source contains read-only page/document Rust APIs and CLI/MCP
planning and preview commands. Desktop Replace controls remain unfinished. The
selected 0.50 and completed 0.52 packages predate these changes; those packages
and the launcher remain unchanged. Development verification is separate from
portable or release qualification.

## Document automation

Start with a literal intent file:

```json
{"version":1,"query":"old","replacement":"new","options":{"wholeWords":true}}
```

Run `folio-cli plan-replacement INPUT.pdf --request intent.json`. The report
includes `planId`, complete counts, bounded matches/blockers and `canApply`.
Automation pages are one-based; each page's `matchIndex` is zero-based. Optional
`selected:[{"page":2,"matchIndex":0}]` chooses original Find results. Omit it for
all matches, or use `[]` for none. Duplicate/nonexistent pairs are rejected.
Query and replacement are literal, with no regex or replacement expansion.

Add `expectedPlanId` with the exact returned `planId`, retaining the same intent,
then run:

```text
folio-cli preview-replacement INPUT.pdf --request intent.json --out-dir EXISTING_DIR
```

Preview recomputes the plan, checks its identity and converts exact native edits
to inspected selections internally. It uses the existing batch preview and
returns its normal bundle/manifest and before/after page images. Save the exact
candidate with `save-text-preview`. Blocked/no-change plans create no successful
bundle. Font, fit and preservation checks remain active at preview time.
Planning omits `expectedPlanId`; preview requires it.

MCP tools `folio_plan_replacement` and `folio_preview_replacement` accept the same
fields plus a launch-authorized `documentId`, without request files. Preview
returns a normal `previewId` for Save/Apply/Discard. After Apply, plan again on
the returned document ID; a previous version's plan is stale. Existing expiry,
capacity, credential handling and worker cancellation/deadline apply. Tools
accept no passwords.

A document scan is complete or returns an error. `pages` contains details only
for pages with matches; `scannedPages` counts every source page. Limits include
10,000 source pages, 1,000 matches per page, 10,000 matches/selections per document,
64 changed objects, 32 changed pages and 512 KiB actual native batch strings.
No-op selections consume no changed object/page capacity. Any selected blocker
suppresses all executable edits; excluded blocked rows remain visible. Overflow
never returns a truncated successful Replace all. Requests are bounded at
512 KiB and final CLI/MCP responses, including envelopes, at 2 MiB.

## Native interfaces and identity

`PdfEngine::plan_text_replacement_page` provides the individual page API below.

The API accepts an immutable native source ID, zero-based page index, literal
query and replacement, document Find options, and optional zero-based match
ordinals. Omitting the ordinals selects all page matches. An empty list selects
none. Unknown or repeated ordinals are rejected. Matching uses the same pinned
Unicode policy as [document Find](document-find.md), including page-wide word
boundaries and phrases across separate text objects.

The result retains every bounded match, its displayed physical-point rectangles,
selection state, and a blocking reason when one native edit cannot represent it.
Mapped source ranges contain object paths and **UTF-8 byte offsets in the logical
source text**. Those offsets are not JavaScript UTF-16 caret positions. They do
not become authority when copied into a caller-authored request.

The result's grouped `edits` use the separate native extracted `expectedText`
identity required by `replace_text_batch`. For example, PDFium can display a
generated space in `Alpha Beta` while the encoded source is `AlphaBeta`; a
replacement of `Beta` may produce logical `AlphaNew` while still checking native
`expectedText: "Alpha Beta"`. A phrase containing the generated separator stays
visible but blocked rather than encoding an invented space.

Native object ownership establishes identity. Geometry only reconciles copies or
distinguishes invocations already associated with an actual native object. When
Find uses the original native reading page, authoritative character ordinals
retain source ligatures even when several letters share identical boxes.
Uncertain copied-reader mappings stay blocked. Both copy and edit permission
are required, including unchanged replacement text.

`sourceFingerprint` identifies the immutable **native working snapshot**. A
protected file's encrypted on-disk bytes have a separate checksum; do not replace
the native fingerprint with a file checksum. A consumer must retain the source
ID/version and validate the returned identity before offering an edit.

`canApply` means a nonempty candidate edit list is available. It is not a validated
PDF preview or a guarantee of replacement font coverage, fit or publication.
`replace_text_batch` still performs those final native checks. Selected blocked
matches suppress the executable edit list atomically. An explicitly chosen
eligible subset can be planned while excluded matches and their reasons remain
visible. No-op plans return no changed edits.

Current limits include 1,024 UTF-8 bytes/query, 4,096 bytes/replacement,
250,000 reading entries, 1,000 matches/page and 2 MiB serialized page results.
Native batch limits of 64 changed objects and 512 KiB expected/replacement bytes
also apply. Exceeding limits fails or reports a batch blocker without a truncated
successful plan. Whole-object deletion and inserted line breaks require further
removal/paragraph integration. Existing source-font eligibility remains in force;
an exactly mapped supplementary character can still be ineligible for editing.

`PdfEngine::plan_text_replacement_document` accepts the same source/query/options,
optional `TextReplacementSelection {page_index,match_index}` pairs, and a
`ReplacementPlanningControl`. Native page indices are zero-based. It returns
physical-point page plans and complete counts without registering a candidate.
Native document JSON is bounded at 4 MiB, with each page retaining its 2 MiB
bound. Progress reports `(completed,total)` at start and after each page;
`cancel()` is checked before and after pages, without interrupting an individual
PDFium operation. CLI adds no progress-stream flag; MCP uses its owned worker
process cancellation.

This API registers no candidate and changes no source bytes. A later desktop
consumer must bind the current workspace, including reordered/imported/duplicated
pages, overlays, crops, fields and bookmarks, and retain preview/Cancel and Undo.
It must not use a backing source that omits current edits or the metadata-removing
redaction preparation route. Physical input, broad compatibility, performance,
packaging and commercial parity are separate qualification work.
