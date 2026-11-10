# Positioned text arrays inside reused PDF objects

## Intent and evidence

Continue the user's autonomous quality loop by making existing PDF text editable
when a nested Form XObject uses a `TJ` array instead of a single `Tj` string.
Current top-level editing already preserves numeric TJ adjustments at logical
character boundaries in `positioned_text.rs`. Nested-form admission, object-path
validation and chain cloning still recognize only Tj/Do. This rejects ordinary
kerned text even when all its fonts, geometry and other state are supported.

The previous turn made verified progress: source-state preservation and safe font
substitution passed 181 native/861 frontend tests, independent readers, scoped
review and Windows acceptance. Preserve its executable and evidence under
`artifacts/form-text-state-20261024`. The overall goal remains active.

## Approach (implementation choice under standing authorization)

1. Regenerate a plain string or flatten the form: simpler, but loses source gaps
   or shared structure and contradicts fidelity requirements. Rejected.
2. Copy the existing TJ rewrite into form editing: preserves the established
   semantics, but risks divergent boundary handling. Rejected in favor of sharing.
3. Extract a bounded, encoding-aware array rewrite used by both existing routes;
   extend form object mapping/cloning to Tj/TJ while preserving original operators.
   Chosen. Retain all existing publication proofs and exact no-op comparison.

No new UI or dependencies are needed. The normal occurrence editor remains the
entry point. Ordinary glyph-bearing TJ arrays support the same original-font and
explicit-substitution workflows as Tj when the existing layout checks pass.

## Editing semantics

Keep source numeric adjustments at their original logical character boundaries,
matching the existing top-level behavior. A shorter replacement drops adjustments
at or beyond its end. Leading adjustments remain. A longer replacement fills the
original slots and appends remaining encoded characters after the original slots;
an old trailing adjustment therefore precedes newly appended characters. Exact
same-text edits preserve all adjustments, including trailing ones. This preserves
explicit source placement, not newly computed kerning for changed letter pairs.

All string segments must end at complete source character boundaries. Boundary
maps are produced by the verified original encoder and the chosen replacement
encoder. Never treat UTF-8 byte offsets as character positions or split an
Identity-H two-byte code. A selected new font may change encoded code width; the
logical boundary indices remain the same. Keep integer/real adjustment values and
their order; do not sum, normalize or round them.

Source string bytes must match the verified source text exactly. Reject malformed
operands, nonfinite adjustments, magnitude above1,000,000, split codes, ambiguous
native operator/object mapping or extraction. Bound source TJ array elements to
16,384 and concatenated encoded bytes to8192 for forms; keep existing top-level
page/text bounds. The shared rewrite must validate boundary maps before slicing.

Numeric-only TJ (including an empty array) changes the text cursor without adding
a native text object. Validate its finite bounded operands, preserve it, and do
not increment an object path or consume the one glyph-bearing show allowed per
BT/ET. It is admitted only inside a text object. Arrays containing empty strings
but no encoded text are conservatively refused in the form route until native
object semantics can be established; this must not accidentally classify mixed
or malformed arrays as numeric-only. Preserve cursor arrays before/after a show.

Keep one glyph-bearing Tj or TJ per BT/ET, explicit local Tf and Tm/Td, supported
Latin standard/simple TrueType/Identity-H fonts, and the existing source-state,
affine, permission, clipping and graph limits. General quote operators, multiple
glyph-bearing shows, paragraph reflow and generated-space inference are separate
compatibility work. Large numeric gaps may still produce ambiguous extracted
text and fail the existing exact-source proof; do not manufacture whitespace.

## Native integration

Add a small shared array rewrite helper. It consumes original array values,
verified expected/changed encoded bytes, source/replacement character-boundary
maps, and whether Unicode text is unchanged; it returns rewritten values. Keep
top-level mapping and policy in positioned_text.rs. Its existing output semantics
must be unchanged on accepted inputs.

Each form occurrence retains its original text-show Operation as well as combined
encoded bytes and state at the show. Use one consistent glyph-bearing-show
classification in flattening, native-tree verification and resource-chain cloning.
Validate each segment against source code boundaries during listing as well as
replacement. A malformed source must not be advertised as editable.

Compute the replacement show from the selected run's verified encoding and the
optional installed font before chain cloning. Pass a complete Operation through
the cloning path rather than always creating Tj. Actual no-op cloning uses the
original Operation. Preserve private flattening only as a validation instrument;
published documents retain their forms and unrelated operators.

Existing nonzero-Tw/single-byte new-font restriction remains per occurrence and
natively enforced. Exact verified program/encoding reuse remains available.
TJ numbers operate in text-space units independent of source byte width, so they
can be retained with a new font while source logical boundaries are re-encoded.
No automatic font choice or silent fallback.

## Verification and acceptance

- RED native fixtures demonstrate original TJ refusal. Shared rewrite unit tests
  cover positive/negative/zero/leading/trailing/consecutive gaps, shorter/equal/
  longer text, empty segments, two-byte source codes, target-width changes,
  malformed maps/operands, and unchanged arrays.
- Native generated nested/shared fixtures cover standard/simple/CID fonts,
  original/substitute fonts, scoped Tc/Tw/Tz/Ts, numeric-only cursor arrays,
  sibling/path preservation, affine/rotated/CropBox/UserUnit geometry, stale
  identity, split-code/ambiguous refusal, clipping/collision, recovery/protection.
- Regress top-level positioned editing and existing form state/font/affine routes.
  Preserve actual no-op glyph/raster, original/private-flat, other occurrences,
  all ancestor boxes and outside-region pixels. Do not relax comparisons.
- Independently authored output references use the specified logical-slot rule
  without calling the production helper. MuPDF and pypdf check saved pixels,
  extraction, retained source positioning and untouched pages/occurrences.
- Fresh scoped review, fingerprinted build and owned Windows actual text-target,
  edit/undo/redo/save/reopen acceptance. Preserve old executables and originals.
  Keep broader script/corpus/performance/hardware gaps explicit and goal active.

Reference: ISO32000-1 section9.4.3/table109 and9.4.4, available from
[Adobe's PDF specification archive](https://opensource.adobe.com/dc-acrobat-sdk-docs/standards/pdfstandards/pdf/PDF32000_2008.pdf).
The earlier developer.adobe.com document-services URL returned404 this cycle.

## Builder choices

Choose internal helper names and the smallest nonredundant fixture matrix that
covers these boundaries. Existing worktree, inline implementation, test/review
workflow, no routine approval questions, no commits or external release are
standing session choices. Record material deviations and verification limits.
