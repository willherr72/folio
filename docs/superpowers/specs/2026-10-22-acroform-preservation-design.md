# Preserve AcroForms before adding form filling

The user authorized autonomous work toward everyday Adobe/Foxit parity. Form
filling remains the next functional subsystem. Inspection found a prerequisite
data-loss bug: PDFium page import retains widget annotations but drops catalog
AcroForm definitions. A two-page native probe retained four widgets and lost all
four recognized fields. This milestone fixes that boundary; it does not claim
field-entry UI or full form parity.

Reconstruct the form from original immutable sources into the private export PDF.
Keep supported AcroForm field hierarchy, shared widgets, values, default values,
flags/options, appearance streams, default resources and calculation order. Prune
widgets on omitted pages. Duplicate pages receive distinct widgets belonging to
the same field. Page order and rotation do not change field identity. Existing
annotations and page content remain under the current export pipeline.

Use a dedicated bounded lopdf graph copier. Allocate retained field/widget IDs
before copying resources, remap page/field references, and never copy source page
trees or catalogs as ordinary resources. Replace imported widgets in place with
the correctly linked widgets and set their P/Parent references. Split merged
field/widget dictionaries into a field parent and annotation children as needed.
Preserve opaque actions; Folio does not execute them. Missing/cyclic field graphs
and malformed structures fail before the destination is published.

For combined PDFs, retain names when unambiguous. Conflicting root field names
get deterministic unique names; reject such a rename when scripts exist because
script references cannot safely be rewritten. The same restriction applies to
field-target actions such as ResetForm, SubmitForm and Hide. Check effective names
below unnamed roots too; an added unique parent name can disambiguate ordinary
forms. Merge default font resources,
rename colliding font aliases, and rewrite DA font operands consistently. Retain
other default resources when nonconflicting; report unsupported collisions.
Calculation order includes only retained fields. Dynamic XFA and already-signed
signature fields need dedicated handling and must report an explicit export
error instead of silently corrupting those structures. Unsigned signatures remain
fields. These limitations are temporary parity gaps, not completion criteria for
the overall project. These refusals also affect the current export-based printing
path and require a later raster-only solution for those document types.

Preserve catalog JavaScript name trees and document open/additional actions needed
by field scripts; refuse conflicting definitions/actions when combining documents.
Folio does not execute scripts, and complete calculation semantics are not certified.
Hide references to merged widgets target the emitted annotation objects, including
duplicates. Named local GoTo destinations resolve to explicit page references;
actions targeting omitted pages become null/absent. Targets outside retained form
widgets that cannot be mapped safely report an explicit error.

Bound all source form traversals per export to 4,000 nodes and emitted field/widget
placements to 4,000 before PDFium page copying. Bound traversal to 64 parent/graph levels,
100,000 copied resource objects and 64 MiB copied resource payload per export.
Read at most the existing 512 MiB source limit and use the existing 64 MiB
decompression cap. No source or destination modification on failure.

Source-text editing currently produces isolated page copies. Preserve forms in
those copies as well. Reject editing a page whose fields have widgets on other
pages until source-version identity can keep those relationships linked; explain
the limitation rather than silently separating a shared field. This is narrower
than the eventual full-editor objective and remains recorded as a gap.

Acceptance: native export/reopen with text, checkbox, radio, choice, hierarchical
and shared fields; reorder/subset/duplicate/append; default-font collisions;
cycles/missing nodes/signed/XFA rejection with untouched destination. Independent
MuPDF and pypdf recognize fields and agree on values; raster appearance equals
the original matching pages. Existing overlays/bookmarks/recovery remain intact.

Alternatives considered: widget-only root repair loses inherited values/resources;
flattening forms removes interactivity; native form setters alone cannot repair
the export boundary and do not regenerate all appearances. Source graph transplant
preserves the existing form before a subsequent filling implementation changes it.
