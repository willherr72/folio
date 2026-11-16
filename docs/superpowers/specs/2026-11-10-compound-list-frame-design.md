# Compound list frames: first compatibility stage

Standing user direction authorizes autonomous implementation and opts out of routine design approval questions. This is the agent's proposed scope, not a claim that the user chose these implementation details.

## Intent and evidence

Fill list boxes whose marked appearance contains a provable static border, an inset clip and option text. The completed dropdown candidate at source `68b376bc56ec3531e4fef8a2e4b52c9bfc1299829d75f1bda54c2dc4933bfcbc` refuses all eight hash-bound cases in `artifacts/list-source-corpus-20261110/current-cli-probe-2/result.json`. Four plain cases fail before any answer publication: original-single, empty-single, multi-one and multi-two. The empty case reports the fallback unidentified-paint error; others report distinct clipping regions. Inspection recognizes all eight as editable list fields.

The source program wraps its border, inner clipping and option text in `/Tx`. Capture sees the border and text in distinct clips. `form_answer_compound::normalize` currently excludes List, although the existing static-frame proof can distinguish a border from the plain option-text region. Selection paint needs a separate proof and must not be classified as an arbitrary static frame.

## Approaches

1. **Small first stage, selected by the agent:** apply existing frame normalization to plain compound Lists only when existing capture fails. Preserve already capturable marked List slots. Keep strict static-paint admission, so source row highlights and ambiguous artwork still refuse. This unlocks four independently reproduced cases with a small private change and no option/font metadata API.
2. Full source-bound row-paint repair: bind actual visible option labels, selected indices, per-widget inherited top index and appearance font resources. This can unlock painted/scrolled cases, but needs a larger private evidence boundary and budgeted font/layout proof. Follow after the plain-frame stage.
3. Whole appearance regeneration: loses the preservation guarantee for arbitrary static artwork; reject this approach.

## Behavior and constraints

Single- and multi-select plain authored fixtures must fill, clear, refill, save, reopen and re-edit while preserving options, defaults, flags, widget geometry, source bytes and exact pixels outside the original inset. Repeated equivalent states must not accumulate obsolete appearance graphs. Already valid generated selection paint must remain usable on re-edit.

The four source-painted/custom-art cases remain refusals in this stage. Do not infer selection identity from paint geometry, erase unproved internal rectangles, or widen clipping. This stage is not proof of generic list scroll/paint compatibility.

No public schema, frontend, dependency, version, cache/history or font-policy changes. Keep 4 MiB appearance bytes, 262144 operations and 64 MiB appearance-copy limits. Do not introduce another full-document parse or the stricter choice-editor 2000-option/128 KiB admission into ordinary filling. Existing source/generation ownership, font/resource context, stale-request and permission guards remain.

Use the existing linked worktree and D: native cache/temp, one Cargo job, serialized tests/builds/UI. Preserve accepted039 launcher, completed compound-dropdown evidence, source68 before copies and all failed probes. No commit, push or package promotion during this stage. Capture new source/input manifests rather than representing the historical source68 contract as checking a changed working tree.

## Acceptance

Meaningful native RED/GREEN for plain List parsing and four engine fixtures, negative paint/clip/state guards, unchanged Text/Combo behavior and generated List selection regression. Full native correctness retains optimized integration and Folio opt-level 0 library meanings. Qualify unchanged frontend evidence separately from the fresh normal GUI/CLI build. Independent saved appearance checks must prove actual visible option labels and selected rows, including empty selection, rather than trusting `/V` or a changed raster alone. Exercise actual desktop selection, Apply/Undo/Redo/save/reopen and owned cleanup. One final source review; disclose parent helper verification separately. No general producer, printing, performance, commercial-parity or portable-release claim.
