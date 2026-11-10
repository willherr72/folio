# Nested existing-text and paragraph automation

## Intent and evidence

Expose Folio's existing native editing of text inside reusable PDF objects through
CLI/MCP. This advances the user's explicit priorities: faithful existing-PDF editing
and useful automation, within the ongoing Adobe/Foxit quality goal. A PDF Form XObject
is reusable page artwork; it is not an AcroForm field or a filled answer.

The user authorized autonomous work without routine design/permission questions.
The choices below are coordinator decisions under that authorization. Implement
inline after the active full native regression finishes, then request one final
independent scoped implementation review. Do not modify frozen product/test inputs
or run a second Cargo while session13326 is active.

Authoritative baseline evidence:

- `artifacts/nested-text-automation-20261027/baseline-1`: a three-page synthetic PDF
  with a nested two-line paragraph reused twice on page1 and once on pages2/3.
  MuPDF extracts all text, but current CLI inspection returns only the page heading
  on page1 and zero runs on pages2/3. Source SHA256
  `2b2ff1e3171dcc61cb25483c81ae2775763cff316768bec365b81bb5f10cb742`.
- `native-probe-1` in the same root: the current candidate's native reusable-object
  listing, paragraph promotion and suggestion succeed on all three pages. This is
  read-only capability evidence, not new edit/save acceptance. Owned process stopped.
- `baseline-inner-1` and `native-probe-inner-1`: the outer object invokes the same
  inner object twice, with adjacent lines. Native suggestion groups four lines
  across those two inner uses on all three pages. This establishes the native
  outer-occurrence boundary; it is not evidence of an accidental sibling edit.
- Existing native implementations: `occurrence_form.rs`, `form_paragraph.rs`,
  `form_paragraph_projection.rs`; current automation only resolves page-level
  `EditableTextRun` through `automation/inspection.rs`.

## Approaches

1. Mix nested runs into existing `inspect-text` by default. Convenient discovery,
   but changes existing ordering/pagination and makes old record assumptions unsafe.
2. Add dedicated nested discovery, then dispatch existing text/paragraph commands
   by a source-bound selection kind. **Chosen:** explicit, backward-compatible
   discovery, shared native proofs and the established preview/session lifecycle.
3. Add a separate editing/publishing engine or flatten the whole input before edits.
   Rejected: duplicates mature preservation logic and changes unrelated content.

## User-facing behavior

Add CLI `inspect-nested-text INPUT.pdf --page N [--offset N] [options]` and MCP
`folio_inspect_nested_text(documentId, page, offset=0)`. Help/tool descriptions
explicitly distinguish reusable page artwork from fillable fields. Existing
`inspect-text` and `inspect-forms` behavior, ordering and identities remain unchanged.

Nested discovery returns the same page/physical-coordinate/source-fingerprint/
pagination envelope as text inspection, with `selectionKind:"nested"` and native
FormTextRun records plus stable `selectionId`. Preserve exact text, object path,
font/physical-size metadata, supported/substitution flags and reason. Unsupported
records remain inspectable; an empty/refused native listing retains its reason.
Do not invent eligibility: native single-run support does not prove paragraph support.

The returned IDs are accepted by existing `preview-text` and `inspect-paragraph`.
Paragraph requests use the existing `preview-paragraph` schema and original palette
style IDs. Existing fitting, bounds, alignment and line-spacing behavior remains.
No font substitution, arbitrary object-path input, new rich-text controls or password
channel is introduced by this milestone. Native compatibility restrictions stay visible.

CLI continues using explicit saved-file paths and exclusive new output paths. MCP
continues operating only on its authorized immutable document snapshots. Both use
the existing exact preview bundle, before/after images, Apply/Reinspect/Save/Discard,
expiry, cancellation, quotas and ownership cleanup. A successful Apply may promote
the edited paragraph into ordinary page text; callers re-inspect both discovery
routes on the returned document and must not expect the old selection kind or ID.

## Identity and native dispatch

Nested selection IDs: `nested-text-v1-` plus lowercase SHA256, domain separated from
page text. Hash the source bytes fingerprint, 1-based page and bounded serialized
native FormTextRun, including its complete path. Never hash worker/source UUIDs.
Every worker re-lists and re-hashes the original source, requires exactly one match,
and gets path/expected text from that match. Client-supplied indices/paths are not
authority. Stale, cross-page, different-source-byte and cross-kind IDs refuse before
mutation. Byte-identical authorized copies intentionally have the same selectors;
these are content identities, not session-document capabilities. Copy/edit rights
and MCP document authorization are still checked on the addressed document.

Single-run nested preview uses `replace_form_text_occurrence` with the complete
source page order, selected page index, resolved object path and expected text,
replacement and no font override. Existing `replace_text_occurrence` serves page IDs.
Keep native copy/edit permission checks, glyph/placement/collision proofs and
complete-source publication. Return the original selected nested run's exact native
text as oldText; do not trim or normalize its identity.

For nested paragraph inspection/resolution, re-resolve the nested seed, call
`list_form_paragraph_runs(source,page,path,expected)`, locate the returned anchor
index in its promoted runs, then call `suggest_form_styled_paragraph` with that
anchor's exact text. All projection IDs remain worker-owned and temporary.
Membership/style data comes only from the newly proved native suggestion.

Paragraph scope is the **outermost page Form invocation**, identified by the first
element of the seed path, matching the existing desktop/native paragraph projection.
A native geometric paragraph suggestion may span sibling inner Form invocations
within that outer occurrence. A PDF's inner resource boundaries are not necessarily
paragraph boundaries. Automation inspection presents the complete suggested text,
styles, bounds and membership; preview replaces exactly that source-bound inspected
paragraph. It does not promise innermost-invocation-only paragraph editing.

Make this visible: nested paragraph inspection returns
`occurrenceScope:{kind:"outer-form",objectPath:[outerIndex]}` alongside the paragraph.
Nested paragraph preview metadata carries the same field. This describes the
maximum grouping scope, not a request to replace the entire outer object. Only
actual inspected paragraph membership may change. A single-run nested edit still
targets just its exact selected run. The API does not accept caller-supplied scope
or membership overrides. Recompute the scope from the resolved seed in every worker.

Keep existing page paragraph IDs unchanged. Nested paragraph IDs use `paragraph-v2-`
and a distinct hash domain containing source fingerprint, page, full nested seed ID
and bounded native paragraph metadata. Including the seed distinguishes identical
text/geometry at different reusable-object paths. Pairing a nested seed with a page
paragraph ID, or a page seed with a nested paragraph ID, is invalid.

Nested fitting/preview dispatches to `fit_form_styled_paragraph` and
`preview_form_styled_paragraph_occurrence`. Page dispatch stays unchanged. The request
uses regenerated paragraph membership/text/styles plus caller runs and controls.
Only the inspected paragraph membership within the chosen outer occurrence changes,
even if another outer occurrence shares every source object. All text outside that
membership, other outer same-page occurrences and other pages retain text and pixels.

## Transport and bundle compatibility

- Discovery pages: 1..10,000; offset: 0..100,000; at most 100 records per batch.
- Each complete nested run record including selectionId: at most 16 KiB UTF-8 JSON.
  Whole final CLI or MCP response including envelopes, escaped text copy,
  structuredContent, request ID and trailing LF: at most 2 MiB. No truncation.
- Paragraph inspection keeps the existing 256 KiB final-frame cap. Original paragraph
  style IDs remain local to that paragraph and source.
- Edit JSON remains 32 KiB. Single-run replacement remains 1..4096 UTF-8 bytes.
  Paragraph remains 1..1024 runs, at most4096 aggregate UTF-8 bytes, style IDs0..63,
  LF line breaks, finite positive in-page bounds and line spacing0.6..3.
- Preview caps remain8 entries, 512 MiB aggregate, 15-minute TTL; worker timeout 120s.
  No larger source/render/engine limit or private-storage capability is introduced.
- Preview manifest versions1(page text),2(page paragraph),3(form answer) retain
  their strict existing meaning. Add4(nested text) and5(nested paragraph), with the
  same fields and 64 KiB manifest cap used for text/paragraph. Validate the exact
  version/selection-prefix/paragraph-metadata pairing; older readers refuse new
  versions rather than misinterpret them. Existing bundles still save/apply unchanged.
  Version 5 requires the derived `paragraph.occurrenceScope` above, with exactly one
  outer index. Versions 1..4 prohibit it; version2 paragraph metadata stays unchanged.
- MCP schemas accept only declared selector prefixes and reject raw paths/unknown
  keys. New discovery is read-only; existing preview/save/apply annotations remain
  accurate. Keep all returned PDF text classified as untrusted document content.

## Acceptance

1. The frozen baseline fixture gains stable nested selections across separate CLI
   workers. Existing page IDs/results remain byte-for-byte equivalent apart from
   explicitly variable outer data. Empty/unsupported pages return truthful reasons.
2. CLI replaces one nested run, previews it, saves exact verified candidate bytes,
   and reopens for another edit. Paragraph inspection, explicit-area reflow and
   native fitting work through the same shared fixture. Native source bytes stay
   unchanged on success, invalid/stale requests and cancellation.
3. Repeated outer same-page and cross-page uses are isolated. Include one outer
   object reusing an inner Form twice: close lines may join the explicitly inspected
   paragraph; distant lines outside its membership must remain unchanged. Inspection
   and preview disclose the outer scope. Independently compare text,
   nonblank artwork where included and outside-edit/unselected-page pixels with
   MuPDF/pypdf; verify saved notes, links, widgets/defaults and unrelated content.
   Include no-op cases and deliberate wrong-selection/missing-image controls.
4. Nested paragraph IDs distinguish different paths even for equal metadata. IDs
   become stale after changes to source bytes or page; mismatched prefix/version
   pairs refuse. Apply/Save/reopen must not resurrect a stale ID. Byte-identical
   authorized copies and no-op operations retain stable content identity. After
   actual paragraph promotion, discovery finds the new representation and prior
   source selectors cannot edit it.
5. Cover standard/embedded subset fonts, generated separators, rotated/scaled/UserUnit
   pages, supported nested transforms, and explicit native refusals for clipping,
   overlap, malformed/oversized/deep paths and unsupported graph features. Include
   protected copy/edit permission denial and owner-authorized operation.
6. Exercise size limits with escaped/non-ASCII text and both modern/legacy MCP wire
   frames. Limits include newly appended IDs and metadata; no field is silently
   truncated. Existing page-text/form-answer transports and versions1..3 regressions pass.
7. MCP: Inspect → Preview → Apply → re-inspect → Preview → Save, with immutable
   document identity, exact candidate hashes, stale/cross-document negatives, quotas,
   cancellation and private-folder cleanup. CLI/native/transport tests and a fresh
   local candidate are required; independent saved-file checks are separate from
   merely observing a successful JSON response.

## Boundaries and implementation discretion

This milestone bridges existing native behavior, without weakening native admission
or claiming support for every producer, script, transform or page graph. Redaction,
digital signing, physical printing, broad performance work and commercial parity
remain in the product roadmap. This choice does not redefine the overall goal.

The implementer chooses small internal helper names and module decomposition, but
must preserve the names, identifiers, limits, native dispatch and version semantics
above. Prefer a dedicated automation nested-inspection module and a small typed
resolved-selection enum over duplicating the transport/publishing engine. Design
review may identify consequential missing decisions before implementation planning.
