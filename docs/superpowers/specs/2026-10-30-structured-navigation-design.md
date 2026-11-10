# Native navigation in combined structured PDFs

## Intent and authority

Continue Folio's broad usability/fidelity goal by making combined tagged PDFs retain usable bookmarks and internal navigation through Save, reopen and further edits. User standing authorization selects routine implementation choices without permission prompts. Preserve source files, prior checkpoints and unrelated work; no commits or releases in this cycle.

The previous language integration is verified (802 scoped native passes plus3 authored probes, corrected desktop Save/re-edit, independent semantic/raster checks). The current catalog assembler still refuses Outlines, Dests and Names. The worker receives a validated portable bookmark list even when it came from unchanged source trees. Writing that list reconstructs entries and cannot retain original SE associations, raw titles, unknown metadata or action graph identity.

## Approaches and coordinator choice

1. Reuse portable bookmark reconstruction for every composition: smaller change, but loses original graph identity and metadata even for untouched bookmarks. Rejected for default preservation.
2. Preserve and join original navigation graphs, with typed destination binding and explicit handling of workspace edits: selected. It uses existing canonical source-page relocation and keeps semantic metadata and actions intact.
3. Continue refusing all bookmarked tagged combinations: a temporary safety boundary, not the product endpoint. Retain only for unresolved input semantics while the selected adapter is developed.

## Endpoint

Whole source blocks in original page order retain source bookmark hierarchy, titles, styles, open/closed state, structure-element links and valid destinations. Top-level items from each source append in source order, without invented wrapper titles. Named destinations from distinct sources must never redirect one another, including identical byte names and legacy Name versus modern String namespaces. URI/remote destinations stay external; local destinations point to canonical relocated pages with original fit/coordinates/nulls. Native annotations and structure ownership remain preserved.

The desktop workflow combines authored tagged/bookmarked inputs, follows bookmarks and internal links, edits text, saves, reopens and follows navigation again. Original Word+Pictures acceptance remains required, not replaced by simpler fixtures.

## Parts

### Read-only outline ownership and staged join

A new graph-native inspector validates canonical root/item identities, parent and sibling links, First/Last pairs, cycles/repeated nodes, title type/size, style values, signed descendant counts and source-owned SE references. It reports top-level items, ordered nodes and visible counts without decoding/re-encoding original strings or expanding action graphs into PdfValue. Local navigation semantics are checked by the destination adapter before enablement.

A staged merger creates one new Outlines root, changes only original top-level Parent and cross-source sibling boundaries, and computes its visible Count. All original children, attributes, raw strings, action references and structure associations remain intact. Shared metadata references remain shared. Reject malformed hierarchy before mutation. Empty source trees are supported.

### Source-bound destination namespace

Read legacy Catalog Dests (Name keys) and Names/Dests trees (String keys) as separate namespaces. Validate name-tree ordering/limits, duplicate keys, aliases, depth/work budgets and destination shapes/page identity. Never treat an integer local page number as an explicit PDF page reference merely because it fits the page count.

Retain original destination value objects. Merge keys without collisions; preserve noncolliding keys and deterministically qualify collisions against all reserved source keys. Rewrite only proven local destination references at recognized outline/link/action locations, once per canonical shared object. Named remote destinations and opaque string payloads are not local references. Unresolved actions or scripts that could depend on renamed keys require explicit handling or a guarded refusal, never a blind dictionary-key rewrite. Other Names trees remain guarded until their own adapters exist.

The portable bookmark importer must distinguish Name and String destination namespaces too, so desktop navigation agrees with the retained graph. Bound all traversal and allocation; do not introduce a second unbounded whole-source parse.

### Workspace Save intent

Determine whether validated workspace bookmarks are semantically unchanged from each current source (not merely a protected historical origin), using the complete source-block mapping and ignoring transient UI IDs as existing complete-source Save does. Unchanged bookmarks use original graph preservation. Explicit replacements/discards retain the existing validated editor semantics and must be honored even when original outlines are unreadable; they do not authorize losing named destinations still used by page links. The distinction must reach graph assembly before outline admission. Do not flatten the authoritative output through PDFium import.

Additional fidelity of editing unknown bookmark metadata/SE associations is a separate editor-model requirement; automatic unchanged Save must not lose it. Record this limit explicitly instead of representing reconstruction as graph-preserving.

## Language and admission

Bookmark Title is a PDF text string, not page content; page language Spans do not cover it. Keep raw titles under their original default when that default remains effective. Differing-default titles require a demonstrated typed language adapter; do not bypass the existing residual-string refusal or guess language. This limitation remains part of the broad goal and must be visible in coverage/continuation. Named destination keys are identifiers; any qualification is deliberate namespace reconciliation, not text rewriting.

Keep source-language/clipping/version-transition checks, source ownership and final retained-graph/writer budgets. Catalog OpenAction or additional action locations require explicit policy before enablement. The initial adapter must not accidentally admit previously guarded form/OC/name-tree features.

## Bounds and transactions

Maximum4000 outline entries (existing editor limit), depth32 for outline hierarchy,100000 graph traversal entries and1,000,000 checkpoints; title strings at most8194 encoded bytes and portable decoded limit4096 UTF8 bytes when used by the editor. Staged navigation metadata at most16MiB; existing graph/parser budgets remain cumulative. Bound both indirect alias hops and semantic recursion. Check cancellation in loops and before the single staged commit. Failure leaves reusable sources and existing output files untouched; no native publication occurs until all adapters validate.

## Acceptance and primary evidence

RED/GREEN coverage: nested open/closed/empty outlines, alias references, source-specific SE identity, shared destination/action objects, names colliding across sources and between Name/String spaces, null XYZ parameters, malformed arrays/parent/sibling/count/cycles, cancellation/budget refusal, exact unchanged raw title bytes and streams, and explicit bookmark replacement/discard. Verify canonical destinations in saved/reopened PDFs with a reader independent of Folio and click the desktop controls on the final candidate. Preserve exact source pixels and native comments/tag ownership across the authored combined editing session.

Primary reference already read locally: Adobe PDF1.7 section8.2.1/Table8.2 and section8.2.2/Tables8.3–8.5 (printed581–587), artifacts/combined-editing-save-20261030/references/adobe-pdf-reference-1.7.pdf. The reference distinguishes Name Catalog/Dests from String Names/Dests, defines exact destination forms and outline links/counts/SE. No release, complete accessibility, printing or commercial-parity claim follows from a scoped checkpoint.


## Builder-check rulings

1. Current portable projection remains a prerequisite of this Save contract. Do not fake a successful projection or authorize an empty surrogate list. Automatic Save of a graph that cannot be represented by the existing model remains refused; explicit discard/replacement is still available under existing permissions. General preservation without portable projection needs a distinct intent/UI contract and stays required follow-up work. Graph action cycles are refused in this stage, separately from outline hierarchy cycles.
2. Shared local/remote/opaque values use typed-edge copy-on-write when qualification is necessary. Never mutate a canonical string globally. Keep original indirect values for remote/opaque consumers; local consumers with the same meaning may share a staged qualified value. This is an authorized identity split necessitated by source namespace separation. Canonical destination value dictionaries/arrays and their unknown attributes remain retained; only keys/local operands change.
3. Workspace replacement is currently whole-list intent, because UI IDs are regenerated and carry no trustworthy persistent source ownership. Unchanged whole-list requests retain raw graphs. Explicit empty replacement authorizes clearing all outlines. A nonempty changed list may use existing reconstruction only when admission proves that no native-only bookmark metadata/SE/action-target semantics would be lost; otherwise refuse until an origin-bound per-item editing model exists. In particular, do not drop A's native-only metadata because a B title changed. Before inference for complete current-source blocks, inherited defaults must use the current sources, not protected-origin historical outlines; protected-origin policy still controls authorization. Keep existing partial-source behavior separate.

Catalog OpenAction and additional catalog action policies remain guarded in this milestone; do not infer a first-source policy by silently ignoring other sources. Standard source page/link action locations needed for local destination renaming are enumerated by the implementation plan. Root custom metadata that cannot be merged remains guarded; original item metadata is retained by raw preservation.
