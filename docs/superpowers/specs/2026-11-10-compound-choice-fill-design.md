# Preserve compound dropdown appearances when filling

Standing user authorization permits autonomous local implementation and replaces routine design/branch approval questions. This is a focused compatibility fix on accepted0.39 source `d327b9bf85fda671fd3a1d856ecac9752b62754ac660780b9ce74cbdd82f42b9`. The separate page-admission experiment is archived and **not accepted**; it is not a dependency.

## Intent and observed behavior

The accepted CLI lists ordinary and editable PyMuPDF dropdowns as editable, but populated appearances fail with `Form answer appearance: answer uses distinct clipping regions`. Explicitly empty variants fail in the fallback empty-answer parser after that same primary parse refusal. The populated PDFs are original producer output; blank variants clear only value/selection and the old text operand, leaving all graphics instructions unchanged. An earlier PyMuPDF `Widget.update` attempt silently selected the first option and is preserved as a failed fixture attempt. Four valid authored PDFs and immutable accepted-CLI failures are recorded under `artifacts/compound-field-answer-20261110`. Ordinary text and multiline controls already succeed.

The appearance combines a static border and inset-clipped answer within one `/Tx` section. Existing `form_answer_compound::normalize` geometrically proves and separates that frame for text fields, but excludes `AnswerKind::Combo`. Consequently the general answer capture sees the border's clip and the text's different clip as competing answer clips.

## Approaches

Chosen: extend the existing bounded static-frame normalization to combo answers. It already proves frame/background geometry, rejects unknown/interleaved artwork, checks text graphics scope, preserves the original clip, and accounts for copying before allocation. This is the executor's choice under standing autonomy.

Alternative: build a separate dropdown-specific isolator. It could provide a distinct policy surface, but would duplicate the same geometry and clip proofs without a demonstrated need.

## Requirements

- Standard and editable combos, populated or initially empty, can receive supported answers while preserving original frame, clip, unrelated content and source bytes.
- Source-owned bounds and all existing lexical, operation, memory, graph, font, permission, stale-value and publication checks remain authoritative. Keep current4MiB program,262144operation and64MiB copy budgets unchanged.
- Unknown artwork, text that escapes/reenters graphics scopes, interleaved paint and distinct actual answer clips still refuse. No general merging of clipping regions.
- List boxes retain their existing behavior in this slice. Their selected-row paint needs a separate proof; the independently reproduced list refusal remains an open compatibility item.
- Choice options/order, flags, field names, defaults, effective typography and widget geometry remain intact. Repeated fills/save/reopen must preserve answers without accumulating obsolete appearance graphs.
- No frontend/schema/dependency/version/cache/history changes, no physical printing or user-app restart, no commit/push/public release. Accepted039 launcher/package remain unchanged.

## Validation and limits

Observe native regression RED on unchanged production logic before the fix. Use exact authored PyMuPDF fixtures rather than moving border paint into an easier wrapper. Native tests compare all pixels outside the **original inset answer clip**, including the border, at2px/physical point, and verify repeated fill/save/reopen values and stable graph sizes. Keep negative controls for unrelated artwork, graphics-frame escapes, separate clips and unsupported list selection paint.

Run affected form suites and the normal separate native profiles, then build a normal profiling-disabled local GUI/CLI candidate with the unchanged resource tree. Exercise ordinary and editable choices through the desktop and save/reopen, and independently verify CLI/desktop outputs with another PDF reader. Exact unchanged frontend-input qualification may reuse the accepted1517test/226file result; label it as reuse. One final fresh source/evidence review for this fix, with one parent correction pass if needed.

This is conditional dropdown compatibility, not general producer, printing, resource/performance or commercial-parity certification. The unqualified performance experiment remains deferred rather than silently incorporated.
