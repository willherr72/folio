# Preserve inherited resources while filling existing PDF fields

## Evidence and intended behavior

The full native regression fails two page-content replacement tests when their
original parent text field is filled. The packaged CLI independently reproduces
the underlying refusal in three synthetic cases: an unmarked answer with absent
appearance Resources, an unmarked answer with an empty Resources dictionary, and
a marked answer with artwork inherited from page resources. Visually identical
local-resource controls succeed. See
`artifacts/native-regression-20261027-rerun-1/form-context-probe-3/result.json`
and provenance.json; earlier mislabeled/failed harness attempts remain preserved.

The answer splice retains text-state operators from an unmarked BT/ET region.
Its resource proof currently receives only the appearance's local dictionary.
Adding a local XObject category can also hide inherited artwork names. A local-only
proof therefore refuses valid page-inherited fonts/artwork. This is a compatibility
gap in ordinary form filling, independently of page replacement.

Filling must preserve the meaning of retained operators, implicit color contexts,
original field/default-answer settings, linked-widget topology and unrelated page
content. Answer layout continues to come from the existing native field engine.
Do not substitute a font or regenerate the entire original appearance to bypass
the proof. No UI, CLI/MCP schema or dependency change is required.

## Decision

Removing the retained-resource check would reopen artwork loss. Replacing the
whole appearance would erase custom graphics. Bind missing resource categories to
the original widget's page context before transplanting the answer instead.

Respect category-level resolution: an existing local category wins as a whole;
do not fill missing names inside it from another category. When a local category
is absent, preserve the effective inherited page category, including implicit
default color-space bindings. Do not confuse AcroForm/field DR used for future
answer generation with page resources used by the original appearance. Retain
the native engine's generated-answer resources in their existing isolated scope.

Use original graph references and bounded direct dictionary copies. Keep the
existing 64 MiB aggregate appearance-publication budget and traversal/depth checks;
charge direct data before cloning. Avoid copying or reparsing a page resource graph
once per linked widget. Reuse the existing page inheritance helper and a bounded
page-context cache or borrowed page-resource map.

Prepare each distinct widget against its own placement context, so two siblings
sharing an appearance can retain different original bindings without changing
their field parent or another page. A single widget dictionary appearing in
multiple page contexts requires a consistent effective binding for any category
being localized. Do not guess or silently choose its first page. Retain unchanged
local-resource behavior; document any genuinely ambiguous alias refusal rather
than treating different placements as automatically unsupported.

Retain the original normal stream/frame and R/D modes. Replace only the answer
slot and its required resource bindings. Keep existing value witnesses, stale
selection checks, permissions, source immutability and atomic publication.

## Acceptance

- RED public-engine regressions reproduce absent/empty AP Resources and inherited
  artwork. Corresponding local-resource controls prove the input is otherwise
  fillable. Inspect actual source programs to prevent mislabeled marked controls.
- GREEN filled values, exact field hierarchy/default-font bindings, and all linked
  widget placements survive save/reopen and another fill. The original stays exact.
- Independently render nonblank inherited artwork before/after filling and compare
  its pixels; confirm unchanged pixels outside answer regions and on other pages.
- Test category precedence with same-name different fonts/XObjects; nested resource
  users; inherited defaults; linked siblings with different page bindings; repeated
  widget aliases; malformed/cyclic and over-budget contexts. Failures preserve the
  original and publish no candidate. Preserve existing caption/empty-value guards.
- Run the previously failing page_content_replace tests and relevant faithful-fill,
  form automation and OCR suites, then reconcile the complete ordinary native gate.
- Build a fresh local candidate and repeat the packaged CLI form-context cases and
  an owned desktop fill/save/reopen workflow. Keep accepted 0.22 packages untouched.
- One final scoped independent review of this correction and its evidence; address
  concrete findings inline. No release or commercial-parity claim.

The separate OCR fixture with a nonempty logical value but vector-only, unmarked
appearance is not evidence of this resource bug. Preserve its ambiguity policy
until independently justified; do not relax answer ownership to make it pass.
Link-test reconciliation is likewise separate from this product correction.

## Operating constraints

User authorized autonomous implementation; no routine approval checkpoint. No
commits, publishing, destructive cleanup or changes to user PDFs. Native session
80037 is collecting the unchanged full baseline: do not mutate its source/test
inputs or start another Cargo. Preparation in artifacts/docs is allowed. Begin
implementation only after that session is terminal and its final failures and
fingerprints are retained. No broad conformance, accessibility, printing or parity
claim follows from these bounded form cases.
