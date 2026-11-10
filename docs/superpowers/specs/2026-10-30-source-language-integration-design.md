# Source language during whole-source Save

The parent structured-source assembly task is still active. The page-language envelope is independently verified but not connected to Save. This stage must preserve the exact Word-plus-Pictures combined editing workflow while resolving the default-language change on unmarked content. Standing autonomous implementation authority applies.

## Approaches and decision

Changing only catalog/structure Lang loses unmarked page language. Rewriting PDF text strings with Unicode language escapes is insufficient for arbitrary language tags and unknown language, and requires reader/identity compatibility work. PDF 2.0 annotation Lang is useful but upgrading a PDF 1.7 file also introduces annotation-appearance requirements. Do not use either as an unverified shortcut.

Proposed PDF 1.7-compatible path: retain the leading catalog language and ordinary Info policy; retain explicit source language on original structure roots; frame differing-default page content with the verified outer Span; bind previously unstructured annotations to source-language-bearing structure elements through OBJR/StructParent/ParentTree. Original annotation strings, appearances and source content streams remain unchanged. Existing annotation structure ownership and language overrides remain authoritative. This is a coordinator design choice awaiting a focused builder check against the standard's whole-object language semantics.

## Language ownership and retained data

- The output catalog default is the leading source's original language, including unknown. Leading Info therefore retains its original context. Later Info is not the combined document's Info, as already specified by the parent policy.
- Original structure top elements receive the source catalog default only when they have no explicit Lang. Nested Lang and property-list overrides remain untouched.
- Whole page Contents is a single marked-content stream for scope purposes. Preserve original references and encoded payloads; add prefix/suffix streams only for differing source defaults. Do not re-encode original operations or wrap pages as new Form XObjects.
- A page Do invocation represents an entire Form graphics object. Establish the source-default language around its invocation, retaining nested Form structure/marked-content overrides. Do not blindly add an inner catalog-language wrapper to every shared Form: that could overwrite an explicit caller language.
- For an annotation already owned by the structure tree, retain that ownership. For a previously unstructured annotation, add one standard Annot (or Link for Link annotations) element with explicit source Lang and OBJRs for its page occurrences. Assign a fresh nonnegative StructParent key and a matching ParentTree entry. Do not set Marked true or assert complete tagging/accessibility.
- Existing PDF 2.0 explicit annotation Lang remains untouched and must be considered an override; do not silently reinterpret a reserved/extension Lang on a 1.x annotation as standard 2.0 semantics.
- Preserve exact annotation dictionaries except the new structural association. Do not flatten appearance streams, change strings, introduce new appearances or infer natural language from text.

## Admission and unresolved typed text

Language is also defined for PDF text strings outside visible content. The adapter must not claim that a page Span covers these. Before changing a source default, inspect the retained source graph for residual string-valued fields outside the supported, explicitly language-owned categories. Known catalog Lang and structural identifiers are not rendered text; leading Info stays under its original catalog default. Annotation text belongs to its whole-object structure scope only if the builder check establishes that rule from primary references.

Other human-readable metadata/resource strings (for example FontFamily, viewport names or arbitrary extension fields) require an explicit typed rule or a conservative adapter refusal for that source. Do not silently guess that an unknown string is binary or language-independent. This is staged admission, not a narrowing of the broad goal; subsequent typed adapters remain required. Preserve all input files and the previously verified exact authored sources.

## Bounds and transactions

Builder-check rulings: refuse changed-default content whose clipping text is
excluded from the new outer Span; ordinary path clips without language-bearing
text remain supported. Inspect nested Forms separately and refuse unresolved
clipping-text ownership there. Before assigning whole-object ownership to an
annotation, refuse an appearance graph with pre-existing structural ownership
that would require reparenting/nesting. Widgets need their own adapter. Generate
new annotation owners in a separate temporary structure root, then reconcile it
through the existing structure merger so an original RoleMap cannot reinterpret
the new standard Annot/Link roles. Revalidate the final ownership graph; temporary
root objects are private until the whole composition succeeds.

Use existing canonical graph identities and ownership inspection. Bound traversal at depth 64, 100,000 objects/owners, 1,000,000 steps, and aggregate parser/header/graph budgets. Carry parser allowances across pages. Cache inspection of shared content without confusing different page contexts. Check cancellation throughout. Prepare mutations privately and validate resulting ownership before publication; failure exposes no partially mutated reusable source or output file.

Existing root K/ParentTree maps must retain original owner associations and null MCID slots. New owners must not collide with old keys. Preserve sharing and canonical aliases; repeated occurrences of one annotation use one parent with the proper page OBJRs. Existing malformed ownership remains refused. Added objects remain subject to retained graph/writer budgets.

## Acceptance

Tests must first reproduce the current wrong default on mixed tagged/unmarked content and leading Info. Add unstructured Text and Link comments, inherited/explicit language overrides, shared annotation occurrences, nested/reused Forms with caller language changes, missing/null/empty defaults, cancellation, hostile keys/aliases and residual-string refusals.

An independent interpreter must compare effective source/output language on page text, structured MCIDs, whole-object annotation text and retained leading Info. Keep this separate from exact text/geometry/raster checks; matching pixels does not prove language preservation. Repeat native Save and the combined desktop paragraph/image/page save/reopen/re-edit session after wiring. No release, PDF/UA or commercial-parity claim follows from this scoped milestone.


Final review ruling: after the final output version is known, including promotion
for retained structure namespaces, inspect every pre2.0 source annotation context
independently of changed catalog language. Refuse a nonnull annotation Lang that
would acquire standard2.0 semantics. Native2.0 annotation overrides remain intact.
This read-only check precedes publication and follows canonical and linked native
annotation identities within the existing traversal limits.
