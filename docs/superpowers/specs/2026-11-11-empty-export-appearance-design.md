# Native List selection with empty export values

The user wants Folio to continue toward dependable Adobe/Foxit usefulness without
routine approval questions. Combined0.42 acceptance exposed a genuine saved-file
defect: clear a single-select List whose first option is `[empty export, Red
display]`, then select row0. The saved `/I[0]` and `/V()` reopen logically as row0,
but its appearance has no selected background. Do not qualify that candidate or
replace this valid operation with a blanket refusal to make acceptance easier.

## Evidence and causal hypothesis

Preserve `artifacts/portable-042-20261111`, especially
`forms-list/cli-acceptance-1`, `empty-export-positive/cli-2/2`, both preceding
clear/Green states, original failures, native expectations and helper corrections.
The source SHA is `d984195c61b99ed8b9f0f15378fd105bac84ef030afb5300a866ee505b0e8223`.
The candidate uses renderer variantv7/DLL
`cc8468ecf58a22e8936686d8678e30d1d050df8640d9f5e58ccbee1b40259b3f`.

The locked local PDFium source has `CPDFSDK_AppStream::SetAsListBox()` counting
`CountSelectedItems()` before calling `GetSelectedIndex()`. That count returns0
for an empty string value. `CPDF_FormField::IsItemSelected()` already selects
authoritative valid indices first and falls back to its existing value policy.
The initial-generation path uses indices directly. Confirm this diagnosis with
an independent C API fill/save probe before altering library source.

## Approach

Use the field's existing selection predicate for each List row in the native
appearance builder, if the native reproduction confirms the causal chain. This
is a localized PDFium patch through Folio's existing pinned source-build workflow.
Keep the renderer's logical selection policy, glyph layout, resource handling,
frame, clipping and ordinary nonempty selections unchanged.

Alternatives considered: refusing all empty-export sources would lose a valid
workflow; painting selection rectangles in Folio or temporarily changing export
values creates a second appearance/model policy. A localized native correction
keeps logical and visible selection consistent at the source of the defect.
These are implementer judgments under the user's standing autonomy preference.

## Required outcome

- Empty export with a nonempty display label remains selectable after clear,
  save/reopen and re-edit. No selection is distinct from selecting that row.
- Native C API evidence must fail onv7 and pass on the candidate. Cover ordinary
  values, empty export, multiple selection, authoritative duplicate exports,
  scrolled/offscreen selection and invalid-index/value fallback according to the
  existing native selection API. Do not invent a broader selection policy.
- Keep exact raw option/export/display values, `/I`, `/V`, `/TI`, defaults,
  field/widget ownership, fonts, frame and outside pixels. Preserve source bytes.
- Build an isolated renderer candidate first. Preservev7 and all accepted builds.
  Install into development resources only after native probe qualification;
  update every provenance/lock/fetch/verifier/build consumer consistently.
- Add real engine save/reopen and visible-selection regressions. Obtain fresh
  complete native evidence, normal desktop/CLI builds and source-bound actual
  GUI/CLI/MCP qualification after the dependency change. The old2729/32result is
  historical and cannot certify the new renderer through metadata reuse.
- Re-run the existing painted/scrolled/linked List states and explicit unsafe
  refusals. Add9paired-export CLI states,3desktop states and MCP discard, with
  all-glyph/source-settings/owner/selection and72/144dpi outside-RGB checks.
  Direct native expected appearances are frozen before application outputs.
- One final fresh scoped review, one inline consequential TDD correction pass,
  and immutable component evidence with corruption/relocation checks.

## Integration and constraints

The0.42 package stays unqualified. Keep its unfinished ledger and every actual
passing/failed result. After the renderer component qualifies, use a fresh next
combined package (expected0.43) and its full extracted acceptance; do not overwrite
the existing0.42 files or reuse its runtime passes for changed renderer bytes.
The accepted0.41 launcher remains unchanged until a new package qualifies.

Use the existing managed worktree, no commits/push/merge/public release, no user
app auto-launch, no routine permission menus. Serialize native/build/UI work,
one Cargo job, D:cache/temp, preserve the100-action history and original512MiB
native/1GiB process-tree selected memory gates. Original sources and old failures
remain immutable. No new printing/signature/accessibility/complex-script,
peak-memory/latency or commercial-parity claim follows from this correction.
