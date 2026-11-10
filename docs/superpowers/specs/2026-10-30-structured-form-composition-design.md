# Graph-preserving form composition

## Intent

Continue the broad Adobe/Foxit-quality goal by making combined, selected, reordered and repeated PDF pages with ordinary interactive forms save without losing original tags, annotations, field answers or later fill behavior. Baseline: `artifacts/duplicate-tagged-pages-20261030/continuation.json`, source `a890e2b7a554cddc8a890f286ad6017dc4cbf2556f494788f2190eea7d39ae29`. That preceding turn is verified progress. This milestone remains incomplete until public Save and desktop acceptance pass; an internal inspector is only a prerequisite.

The user has authorized autonomous implementation decisions and opted out of routine approval questions. The following choices are the coordinator's decisions, not new user statements. Existing isolated worktree, Rust/lopdf/PDFium and Windows/Tauri stack remain in use. No new dependencies or renderer changes.

## Approaches and choice

1. Reuse `acroforms::preserve` after page import. It already handles field resources and repeated widgets, but replaces widget identities and is designed for PDFium-imported page graphs. Reusing it unchanged would break original OBJR ownership and opaque references. Rejected for graph-preserving Save.
2. Extend graph-native composition with a typed form model, retaining relocated field/widget identities and rewriting only understood relationships. This has more explicit ownership work but retains source tags, appearance streams and resource aliases. **Selected.** Reuse validated parsing/default-appearance/resource helpers where their contracts match; do not copy the old whole-graph repair path blindly.
3. Flatten fields to appearance content. This loses editability, answers and behavior and does not meet the intended outcome. Not a fallback.

Added/imported text, images, ink, highlights and comments are a separate publication adapter after this form work. They remain on the goal roadmap, not silently admitted by this change. Existing overlay rejection remains until their language, persistence and tag ownership can be represented.

## Required behavior

- Preserve field hierarchy, values/defaults/options/flags, independent widget geometry and appearances, calculation order, resource bindings and supported actions. Tagged widget OBJR and ParentTree ownership must still refer to the actual page widget. Retain source streams byte-for-byte unless an explicitly understood default-appearance rename needs rewriting.
- Plain source field names stay unchanged when unique. Deterministically rename only conflicting roots using existing suffix conventions, preserving all descendant fully qualified names and updating typed field-name actions. Refuse conflicts with opaque scripts/name consumers rather than retargeting them by guesswork. Same-name fields from distinct documents must never silently become one answer.
- Selection keeps surviving widgets of a shared field linked to that field. Removing its last widget does not imply deleting a legitimate nonvisual field; classify visual and nonvisual ownership explicitly. Prune calculation/action targets only under the matching supported action semantics, not by generic reference removal.
- Repeated page widgets of one source continue to represent the same logical answer, as in Folio's existing form duplication path. A repeated merged field/widget needs a shared field parent and occurrence-owned widgets. Preserve original annotation/tag identities; typed consumers of the prior merged object must be inspected and rebound where required. If an unknown consumer cannot be proved safe, refuse that case without overwriting the destination.
- Default DA/Q inheritance is materialized before source AcroForms are joined. Merge DR with collision-free font names and rewrite only default appearance font operands, not arbitrary strings or appearance-owned names. Preserve independent widget/page/appearance resource contexts, including later fill. Non-font resource collisions need explicit identity/compatibility handling, never last-writer wins.
- Keep the existing action/navigation walker in charge of destinations. Add typed field-action contexts and target ownership where needed; unknown actions/scripts that depend on renamed names or removed pages/fields remain guarded. No script execution. Preserve calculation order within each source and source order between independent forms.
- Preserve source language for tooltips, field descriptions, option labels, values and appearance text through typed source ownership, using valid existing language scopes. Untagged widgets cannot simply inherit the leading source's language. Do not treat form identifiers or DA programs as prose. PDF version-promotion checks remain.
- Empty/no AcroForm, aliases, null optional entries and nonvisual fields are modeled explicitly. XFA, signed forms and unsupported proprietary catalog semantics remain guarded. Empty AcroForm support must be deliberate, not an accidental bypass.
- Builder-check ruling: effective NeedAppearances must agree across contributing form-bearing sources (absent/null means false). Conflicting policies are refused until a separate appearance-normalization/proof adapter exists; OR-ing them or clearing true would change another source's behavior. Sources without forms do not vote. Preserve agreed policy and prove appearance behavior in acceptance.
- Builder-check ruling: differing-language field prose is admitted only when a valid identified PDF scope demonstrably covers it. A widget OBJR is not by itself proof for a shared parent field's strings or a nonvisual field. Until a typed representation proves that case, preserve the existing differing-language refusal. Same-default-language form combinations remain the principal new path; identifiers are decoded separately from prose. Do not invent a field-level Lang key or silently pass prose through the language walker. These are recorded compatibility limits, not a completion claim for broad language support.
- Save authorization, encryption, cancellation and atomic publication remain unchanged. No source document or existing output is mutated by refusal/cancellation.

## Architecture and sequence

### 1. Read-only form inventory

Add `composition_forms.rs` with a bounded canonical identity inventory for a source catalog and validated `SourcePages`. It records form presence, roots, field nodes, parent/child topology, explicit/effective names and types, widget-to-page and widget-to-logical-field ownership, and calculation-order identities. It returns references/IDs plus bounded small strings; it never clones appearance streams or resource graphs. A field tree and page Annots must agree bidirectionally; ambiguous ownership, cycles, missing references, duplicate fully qualified names, mixed field/widget children and orphan widgets are refused. Canonical aliases are resolved before identity comparisons. Page-less nonvisual fields remain in the inventory. Inventory alone does not authorize publication or validate every field appearance/action.

Use a budget shared across source inventories: at most 4,000 field/widget nodes total, 100,000 inspected values/steps, depth64 and 16 MiB staged metadata. Charge before copying/decoding names, and reject unsupported encodings rather than treating unequal bytes as distinct equal names. Existing 100,000-object / 1,000,000-value / 256 MiB retained-document limits remain. Check cancellation throughout alias and array traversal. All source dictionaries and stream bytes remain untouched.

### 2. Source defaults and catalog merge

Add a separate form-assembly module consuming inventories. Stage field root DA/Q materialization, DR/DA aliases, field-root name changes, CO and form flags against the private imported graph. Preserve identities for original nodes/widgets and unaffected values. Validate before committing; use cumulative direct-copy and retained budgets. Join AcroForm only after source language/navigation checks can account for form contexts. Keep public admission closed until the next phases exist.

### 3. Projection and occurrence integration

Extend typed projection with field/widget membership updates, safe removal, calculation/action target changes and opaque-consumer guards. Extend sparse occurrence planning with repeated widget ownership and merged-field normalization while maintaining original tags. Reuse the existing copy budget and typed action cache; do not generic-remap field names, page targets or source graph dictionaries. Validate the resulting field/page/structure graphs together after projection.

### 4. Language, actions and public Save

Integrate inventories into source-language analysis, annotation ownership, destination consumers and catalog assembly; broaden only the validated AcroForm admission. Add a final inventory of the output to prove hierarchy/widget/page consistency and bounded retention. Connect the full route through `structured_source_save`, keeping overlay guards. Reopen and fill a saved field to verify default font and appearance behavior rather than relying only on dictionary equality.

## Verification and closure

First reproduce a public tagged-form combination refusal. Use native fixtures covering common text, checkbox, radio, choice and nonvisual fields; shared widgets; same-name independent sources; distinct defaults/resources; tagged and untagged widgets; source languages; indirect aliases; selection/deletion/reorder/duplicates; protected inputs; cancellation; limits; malformed/cyclic/mixed ownership and external typed/opaque actions.

Meaningful native model tests precede implementation. Independent pypdf/PyMuPDF audits must compare full field/value/default/options topology, tag/OBJR membership, original appearance bytes and 1x/2x pixels; after a requested fill, compare the intended value/appearance and unchanged surroundings. Include damaged graph/value/font controls with a passing unchanged rewrite. Fresh TypeScript/Vite and GUI/CLI builds and owned Windows Add PDF → fill → Duplicate/Delete/Undo/Redo → Save/reopen → fill again acceptance are required before milestone closure. Preserve originals, failed evidence, frozen baseline and old packages. One builder design check and one final scoped implementation review; coordinator resolves small findings with tests, no review churn. No all-target/release/printing/accessibility/commercial-parity claim from partial evidence.


## Native button-copy correction (2026-10-31)

Native controls-red-1 reproduced four real later-fill failures: indexed checkbox, indexed radio (with/without unison), and plain non-unison radio. Plain checkbox, plain unison radio, dropdown and list passed. Original-source fill is a passing prerequisite for each case; copied controls were compared individually and against the original filled raster. Prior dictionary-only button evidence does not establish later-fill fidelity.

Coordinator ruling under standing autonomy: the shared-answer requirement governs representation. Keeping every original Ff/Opt byte is incompatible with synchronized repeated controls. Keep ordinary selection remapping unchanged. For duplication only, use a separate staged button adapter. Plain non-unison radios may set only RadiosInUnison after proving original on-state names are distinct, so original options retain exclusive behavior and only their new copies synchronize. Indexed buttons may convert positional Opt/AP/AS/V/DV to named export states when every original export is distinct, nonempty, not Off, and losslessly representable as a PDFDocEncoding name within 127 bytes. Retire a local Opt only when no nonnull ancestor Opt exists, retain logical exports/current/default answers, rekey normal/rollover/down dictionaries without copying appearance streams, and enable radio unison for the copied equivalents. Preserve all other flags. Do not alter the renderer, native filling, source files, tags or widget identity.

Reject ambiguous original equivalence classes, unsupported export encodings/names, inconsistent checked/value/default states, malformed appearance roles, and budget/cancellation failures before publication. Already named checkboxes/unison radios need no representation conversion. A saved converted field must survive later duplication and page selection. Wider Unicode/indexed-ambiguity support remains an explicit compatibility gap; do not claim all forms supported.

Primary reference: Adobe PDF1.5 table8.67 (https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.5_v6.pdf) requires positional on-state names with Opt; keeping old numeric names on appended controls would violate that representation. Bundled PDFium core/fpdfdoc/cpdf_formfield.cpp CheckControl synchronizes unison controls only with matching export and on-state names; non-unison radios select one physical control. Changing native fill alone would not fix other readers. No actions or network destinations are executed.


Inherited Opt correction to the ruling: null is equivalent to an absent PDF dictionary entry, so relying on it to shadow ancestor Opt is not portable. Inherited nonnull indexed export tables (including an ancestor underneath a local override) are explicitly refused for duplication until an ancestor/sibling materialization adapter exists. Ff/V/DV inheritance with a local Opt remains supported. The native inherited-controls-red-1 test demonstrated the missing refusal with a passing source-fill control; it checks destination/source preservation. This narrows only that unproven representation, not the shared-answer goal.


Final-review inheritance correction: button Opt/Ff/V/DV lookup skips direct and indirect nulls before choosing a representation or translating states. An ancestor indexed table remains guarded even below a local null; inherited defaults and radio flags must be translated using their effective values. The signed-field inventory carries borrowed effective V through the bounded hierarchy, so an untyped parent cannot hide a signature value from a typed descendant. These correct the existing preservation/refusal contract; they do not introduce signature editing or broaden inherited-indexed-table support.
