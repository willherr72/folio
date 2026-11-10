# Navigation preservation when pages are deleted

## Intent and authorization

Extend Folio's verified selected/reordered tagged-page Save so users can delete pages that have incoming links, native bookmarks and named destinations. The user has authorized autonomous implementation and routine decisions. Preserve surviving PDF objects and native metadata rather than rebuilding the outline from its reduced UI model. No permission checkpoint, commit, release or portable overwrite.

Baseline: `artifacts/selected-structured-pages-20261030/continuation.json`, product fingerprint `e4a9b65c89716e7e1c0b0a48a23a0239f39147822c0e4ded6b2d0febc01120f8`. The previous goal turn completed a verified checkpoint; this is progress toward the original broad goal, not a new definition of completion.

## Alternatives and chosen architecture

Rebuilding bookmarks from portable fields loses raw titles, producer metadata, SE targets and shared actions. Redirecting missing destinations to another page invents author intent. Dropping all navigation loses usable surviving targets. Extend the existing typed destination/action walker with a selection mode, plus original-object outline topology projection. Reuse its bounded parser, canonical ownership, action context roles, copy-on-write cache and atomic staging.

Graph-native composition first assembles complete current source graphs with qualified destination namespaces and source-language checks. Selection then prunes typed navigation, tags and the page tree in the owned scratch graph. The existing final reachability check still rejects unhandled references to removed pages, tags or outline nodes. No lossy renderer-import fallback.

## Deletion semantics (coordinator decisions)

- A local explicit destination to a retained page stays byte/identity equivalent. A legacy Name operand and modern String operand resolve in separate namespaces; an entry whose canonical target page is omitted is removed from its own catalog namespace. Surviving named metadata stays intact, with valid sorted Names/limits and no unbound retained local operand.
- Keep a link annotation and its geometry, appearance, visual text and tag ownership. Remove only Dest/A when its primary local GoTo target is deleted. Do not retarget to a nearby page.
- Treat a removed GoTo as a removed action branch, including its attached Next descendants. Do not promote follow-up external actions into the former primary action. Within a retained action's Next array, remove only branches whose own local GoTo target was deleted; keep the order and sharing of remaining siblings. This matches the existing portable page-deletion mapping semantics. Remote GoToR names/indices, URI values and supported relative Named actions stay unchanged.
- A leaf outline item whose primary portable local target is deleted is removed. If it has surviving children, retain it as a folder with its original raw title, flags, colors and private metadata; clear the missing Dest/A. Untargeted folders remain. Recompute affected First/Last/Prev/Next/Count topology and signed open/closed descendant counts. Retained original outline object IDs and alias identities are preserved. Deleted SE targets are cleared; SE-only items remain descriptive folders, matching their current portable UI representation. Surviving SE targets remain unchanged.
- Unknown active action kinds, JavaScript, unsupported AA triggers, presentation PresSteps and non-Link annotation action contexts remain guarded when selection changes page membership. Existing catalog and widget adapters remain unchanged. Opaque references to removed objects are still refused. This does not claim support for proprietary hidden destination consumers encoded inside arbitrary producer data.
- Native-only bookmark metadata changes requested independently by the user still require the existing origin-bound editing adapter. A portable list that exactly matches the current source's selected-page projection is recognized as selection, allowing preservation/pruning of original outline objects instead of CompatibleReplace. Explicit discard remains authoritative even if originals are unreadable. Compatible user edits continue through existing checks.

## Interfaces and ownership

- Extend `composition_destinations::Prepared` with canonical target lookup and selected catalog staging. Validation of complete originals precedes filtering. Do not merge Name/String maps or interpret remote operands as local.
- Add a projection entry point around `composition_navigation::Walker`, receiving the original `SourcePages`, original `StructureInfo`, retained page set and removed structure-element set. Return removed outline identities for the final forbidden-reference scan.
- Selection mode in the action walker distinguishes unchanged, rewritten and removed results. Copy-on-write rewrites preserve shared action identities within each resulting graph; untouched actions stay original. Removed values do not leave null elements in action sequences. A cached subtree must still satisfy total depth at each use site; cycles, role conflicts and budget exhaustion refuse before commit.
- Outline projection uses the already-inspected original hierarchy and typed primary target resolution. It stages only changed structural/navigation fields on canonical dictionaries. Original unknown metadata is not copied or discarded as an implementation shortcut. References to removed outline nodes from surviving opaque data cause final refusal.
- Integrate after the structure filter knows removed elements, before final page/tag reachability validation. Retain original structure membership for outline SE admission during this scratch transition. Inspect the resulting outline/navigation against the selected page set again before publication.
- Save intent recognizes current selected projection using the bounded bookmark mapper and internal canonical page keys. Original permission provenance remains authoritative. Legacy per-page recovery provenance is not reinterpreted as a current whole source.

## Budgets and safety

Existing cumulative budgets remain: 100,000 graph objects, 1,000,000 traversal steps/values per bounded pass, 16 MiB staged navigation metadata, action/outline depth32, structure/page depth64, 4,000 outline items. Charge before copying direct dictionaries/arrays; no encoded stream payload clone. No whole-source clone per action/occurrence, no enlarged limits or renderer changes. All work is in an owned scratch graph; cancellation/error leaves destination and open originals unchanged. Outer protection/atomic publication stays in place.

## Acceptance

Reproduce current public-engine refusal using tagged PDFs with (1) a retained link to a deleted page and (2) native raw outline metadata plus distinct Name/String catalogs and shared actions. Write red tests before implementation. Cover target namespace separation, aliased/indirect operands, remote values, retained folder children, closed counts, SE removal, shared action DAGs, removed Next branches versus surviving siblings, unknown contexts, external references to deleted outline nodes, cancellation, depth and aggregate budgets. Retained pixels/text and source files must remain identical.

Use the authored Word/Pictures navigation corpus. Delete a page that leaves the other page's incoming link and a named target, Save/reopen, navigate surviving bookmarks, and independently audit namespace membership, local targets, raw surviving outline fields, action sharing, tag ownership, exact1x/2x pixels and text/images. Add intentional broken-output controls for the new oracle. Fresh source-bound GUI/CLI build and owned Windows organizer workflow, then one final implementation review; correct concrete findings without review churn. No full all-target/accessibility/printing/portable/parity claim from this milestone.

Repeated page ownership, typed table/ID projection and combined forms/overlays remain separate work on the active broad goal.
