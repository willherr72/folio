# Complete-source reusable form text

Continue the active document-fidelity goal. Existing reusable PDF Form XObject
editing rejects Annots/AP anywhere in the source; a shared interactive field also
prevents isolated page adoption. The accepted grouped checkpoint is unchanged at
artifacts/occurrence-group-20261024/checkpoint/verification.json.

Chosen approach: reuse private text-page projection and complete-source publisher,
with an explicit internal annotation-aware form-editor mode. Legacy APIs retain
their admission policy. The original source must pass form graph preflight and
tag/layer/intermediate-resource checks before projection. Never strip annotations
or flatten the published form graph. The alternatives (mutating a complete document
inside all form algorithms, or stripping annotations to evade admission) either
duplicate established publication logic or lose the fidelity proof.

New native methods: list_occurrence_form_text_runs(source,page) -> FormTextRuns;
replace_form_text_occurrence(source,occurrences,selected,path,expected,replacement,
font_id) -> PageContentReplacement. Source geometry is scaled to physical points
on the public boundary, matching existing methods. Original permissions, duplicate
assembly authorization, bookmarks, provenance and immutable input bytes survive.
Private projections/candidates are discarded on success, error and abandoned reply.

The form model already clones annotation dictionaries and reachable resources;
flattening is only a validation instrument. Keep exact baseline/no-op/flat raster
and glyph proofs. Restore native note geometry when creating a validation copy.
Add annotation rectangle collision checks before publishing: a replacement must
not introduce or enlarge overlap with an annotation interaction rectangle, including
the renderer's change fringe. Malformed or nonfinite geometry refuses mutation.
Existing overlap can remain when editing in place; no-op must retain exact pixels.
All existing font/state/advance/affine/neighbor proofs remain required.

Acceptance: authored nested shared forms on a two-page source with shared widgets,
native notes, direct/named/self links and bookmark. Unique, first and later duplicate
edits and no-op, rotations/physical scales, another-page annotations, clipping,
stale input, overlap, tagged/layer refusal and original rights. Compare selected
invocation against an independently authored fixture, untouched invocations/pages
against source; save/reopen, re-edit and fill shared fields. Test worker cleanup.
One final scoped review plus correction pass. Retain failed evidence. This native
milestone is followed by desktop capability/IPC wiring and Windows acceptance;
neither desktop support nor broad parity is claimed by native tests alone.

Evidence-driven validation correction: the generated connected fixture exposes
an existing failure even with all annotations/fields removed and unreachable
objects pruned. At width 1000, exactly three glyph-edge pixels differ by one gray
level; width 600 and 1200 and MuPDF 1x/2x are exact. Restoring page font resources
does not change the discrepancy. Therefore the private flat-validation comparison
may accept only interior-channel one-level differences with identical alpha AND
an exact width-1200/max-height-1920 second rendering, still requiring matching
physical glyphs. Actual page isolation, no-op cloning, outside-edit pixels and
publication stay exact. This is a deliberate validation-rule change supported by
the retained flat-probe evidence, not a general pixel tolerance. Test high-contrast,
alpha, dimensions and two-level differences explicitly. Remove diagnostic export
instrumentation from product code before final verification.

Standing authorization removes routine design approval questions. No commits,
releases, original overwrites or cleanup. Existing worktree remains isolated.
