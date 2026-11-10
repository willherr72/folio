# Inherited paragraph pages plan

- [ ] Author inherited/indirect/override and scaled fixtures; exercise real editing,
  export and saved replay, with independent physical geometry evidence.
- [ ] Diagnose failures and fix the responsible boundary; run affected regressions.
- [ ] Review material changes, build/accept changed app behavior, record hashes,
  preserve sources and report remaining compatibility limits.

Execute inline in the existing worktree. Previous cycle completed a reproduced
multi-style glyph-loss fix,152 native passes,27 independent outputs and Windows
acceptance; this is a new evidence-driven compatibility cycle.


### Next bounded implementation pass: physical font and overlay limits

The boundary and common-size workflows are implemented but not release accepted.
Before a desktop build, reproduce 1pt/300pt paragraph requests under units0.5/2
and distinguish intended physical1..300 policy from independent raw numeric caps.
Audit together: text_paragraph prepare/preview, paragraph_selection.horizontal,
paragraph_memory.read, styled_paragraph_plan changes, styled_paragraph_memory
Style/fragment/read/probes, styled_paragraph_layout metrics, and highlight padding.
Use source-owned page units, never a request's claimed scale. Do not broaden saved
metadata acceptance without independent font/glyph/replay verification. Keep unit1
behavior and existing raw saved records stable. Suggested architecture: a shared
source-page unit reader plus unit-aware size-range checks; thread units through
pure layout/suggestion admission rather than globally replacing bounds with broad
positive checks. Add endpoint/no-op/reopen tests, including source text already at
small raw size and invalid physical sizes. Check font replacement explicit changes
and retained default styles separately.

Overlay limits need the same audit in validate_export, text/ink validators,
TextOverlay.validate_area_settings, and appearance generation (which also validates
raw dimensions). Validate physical policy while retaining independent finite raw
geometry limits. Do not silently change1..300pt or512pt UI promises. General decimal
f32 inversion remains a separate gate: unchanged public bounds/style sizes must
resolve back to independently verified raw source values, without weakening saved
exact geometry proofs or snapping intentional changes by loose tolerances.


### Overlay audit follow-through (after paragraph pass)

Confirmed callers still enforce physical policy on raw values: engine.validate_text
(512pt), validate_ink (100pt); types.TextOverlay.validate_area_settings (14400pt);
semantic_pdf.prepare_text_overlay (512pt), text_area.validate (1..512pt/14400pt),
text_area_overlay.prepare and nested single-line preparation; persistence calls
prepare_text_overlay for generated appearances. Recovery now validates physical
plans and should retain those physical checks. Public prepare/layout APIs likewise
remain physical and must not be loosened. Thread source-owned units through raw
worker preparation, including recursive text-area lines, or prepare in physical
space with a rigorously checked appearance transform. Audit raw numeric limits
separately. Reproduce standard text, shaped custom text, area text, and ink upper
endpoints on units0.5/2; reject physical overlimits on both. Verify saved physical
geometry and editable/flattened/print/recovery paths. No overlay fix claimed yet.
