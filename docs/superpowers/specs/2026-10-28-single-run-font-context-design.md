# Source-preserving single-run font substitution

## Intent and authorization

Let existing text editing explicitly choose a complete font even when an otherwise
editable page uses Word-style clipping, tags or positioned TJ strings. Retain the
same run, baseline, size, color and surrounding document. The user authorized
autonomous implementation without routine design questions; these implementation
choices are coordinator decisions. Preserve originals, dirty worktree and accepted
portable packages. One final scoped review, inline implementation, no commits.

The prior paragraph-font automation milestone is complete. Reconnaissance and two
original synthetic Word operator maps are in
`artifacts/single-run-font-context-20261028/recon.{md,json}`. Word uses nonzero TJ
adjustments; removing the context refusal alone is insufficient. A run adjacent
to other text must still refuse overlapping growth; paragraph editing handles
reflow. Do not present single-run substitution as an arbitrary text-reflow tool.

## Approaches

1. Reuse paragraph publication for a one-run selection. It already embeds fonts
   and proves contexts, but resets text state and can wrap/reanchor the run. This
   would require another single-line placement contract and does not naturally
   preserve existing TJ/Tc/Tw semantics.
2. Extend source-operator substitution with a private, reproducible source-bound
   splice plan. This preserves text-show slots and inherited state and shares the
   native post-publication checks. Chosen because it matches single-run semantics.

Do not normalize a preserved context through PDFium stream regeneration, expand a
run into an unrequested paragraph, trust incoming metadata as permission, or make
pixel equality the sole clipping/tag ownership proof.

## Native architecture

Add a private `text_substitution` module beneath text_edit. A `Plan` owns bounded
source operations and the planned candidate operations/resources. Construction
uses one isolated page, verified source operands, the selected native snapshot,
all native runs and the immutable explicitly selected FontAsset. It cannot be
created from client/PDF hint metadata. Its fields stay private. Source operations,
font resource and size must match the selected native show ordinal. Preserve
q/Q-balanced text state and reject malformed or ambiguous mapping before editing.

The plan selects a collision-free (or independently verified reusable) font
resource, reencodes complete characters, and retains TJ adjustments at their
logical source slots using text_show. Tj/TJ and supported quote operators retain
their line movement and spacing effects. New characters extend the final segment.
Selected font size, baseline, matrix, color and rendering mode remain unchanged.

Identity-H does not apply Tw to a two-byte space. Where source single-byte code32
represents a space, carry its effective word spacing into bounded TJ adjustments
for the replacement rather than silently dropping it. Other text-state values
remain intact. Any temporary state change is restored before unselected content.
Quote operators must retain their original Tw/Tc assignments and line movement.

When subsequent shows depend on the selected cursor, preserve their original
position with a bounded numeric TJ correction based on verified old/new advances,
gap totals and character/word spacing. Keep the text line matrix unchanged. Reuse
existing verified advance semantics; do not infer cursor displacement from ink
bounds. Refuse nonfinite or excessive corrections. Repeated substitutions must
reuse proven fonts and compact only mathematically redundant adjacent selections
or owned correction slots, retaining other source operators.

PublicationProof receives a private plan-derived expected operation sequence only
after matching the complete source sequence and selected ordinal. Exact expected
operations must match baseline/candidate/full-source publication as appropriate.
The original clipping/mark binding and Group, native glyph containment, tag
ownership and graph proof still apply. Both the isolated replacement and outer
complete-source publication need the same substitution authority. Arbitrary caller
operations or unverified saved hints never acquire that authority.

Discovery reports substitution eligibility per selected run using the same source
admission prerequisites. It must not advertise a page-wide guarantee that the
replacement will fit or that any font covers the requested characters.

## Existing safety and resource contracts

Keep Latin/single-line/native transform/rendering constraints and 4096 UTF-8 bytes
for single-run text. Content decoding remains at most16 MiB and100,000 operations;
graphics/resource ancestry at most64. Use finite adjustments bounded by1,000,000.
The existing document512 MiB bound, native serialized worker ownership and complete
source publication remain. Font assets use the existing parser, permission checks,
glyph validation and registry ownership. No renderer patch is planned.

Retain native source/copy/no-op proof, exact other-run properties and glyphs,
outside-edit pixels, visible-page bounds, collision refusal, clipping containment,
comments, complete-source graph and password permissions. Existing no-font edits
and unchanged-save identity cannot regress. A selected font does not authorize
clipped ink, changed tag semantics, unsupported shaping or adjacent content loss.

## Desktop and automation

The desktop already accepts a registered font ID; corrected discovery and native
replacement should make that existing picker usable on eligible contexts. Verify
Preview/Cancel/Apply/Undo/Redo/save/reopen with a fresh executable.

After native proof, extend single-run CLI preview with the same bounded explicit
FontInputs as paragraph preview and an optional strict `fontId` in TextEdit.
MCP uses launch-authorized IDs only, including nested existing single-run routes
where their native substitution contract permits. Record selected font metadata
in the preview, keep old bundles compatible, and disable exact-byte shortcut when
an explicit font is requested. Save/Apply remain self-contained; no paths are
accepted at tool time. No new installed-font scan or silent fallback.

## Acceptance

Native: clipped/tagged Tj, multi-segment TJ with leading/trailing gaps, quote
operators, character and effective word spacing, dependent following text,
font resource reuse/repeated substitutions, missing glyphs, stale source,
permissions, malformed/ambiguous resource state, clip/tag tamper and overlap
refusals. Preserve original no-font and paragraph cases.

Fresh outputs: original tagged and untagged Word standalone-run growth plus a
shorter/same-length accented replacement; overlapping accented growth must refuse.
Check complete expected text, exact baseline/size/color, unselected glyphs/pixels,
second page and nonempty tagged graph. Reopen and edit without external fonts.
Cover desktop and CLI/modern+legacy MCP, font-file mutation/deletion, no-op exact
bytes without override and protected refusal. One final review; correct concrete
findings inline with failing reproductions. Record a local checkpoint, not parity
or release approval. Broader scripts/formats, printing and full crash/performance
certification remain separate requirements in the overall product goal.
