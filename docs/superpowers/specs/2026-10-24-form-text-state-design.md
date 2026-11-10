# Preserve ordinary text state when editing reusable PDF content

## Intent and authority

Improve existing-PDF editing, the user's original pain point. Folio already edits
one nested Form XObject occurrence without changing other placements, including
verified embedded fonts and affine transforms. Its grammar currently rejects even
`0 Tc`, `0 Tw`, `100 Tz`, `0 Ts`, `0 TL` and `0 Tr`. Ordinary producer state setup
therefore makes otherwise supported text unavailable.

The user authorized autonomous implementation without routine design/permission
questions. The decisions below are coordinator choices, not newly stated user
preferences. Retain the existing Windows/Rust/PDFium/React architecture and dirty
isolated worktree. Preserve all source documents and the accepted image-export
executable. This milestone advances compatibility; the overarching Adobe/Foxit
usefulness goal and its other gaps remain active.

## Approaches

1. Ignore only default-valued operators. Easy but leaves real spacing/scaling
   unsupported and does not solve the underlying preservation requirement.
2. Regenerate or flatten source text to normalize state. This can change glyph
   placement, clipping, resource sharing and editability of unrelated content.
3. Preserve source operators and track effective text state through the existing
   occurrence-isolation model. Chosen: this extends the established exact no-op,
   independent-output and surrounding-content proofs without flattening publication.

## Behavior and scope

Accept bounded scalar character spacing (`Tc`), word spacing (`Tw`), horizontal
scale (`Tz`), text rise (`Ts`) and leading (`TL`), and explicit filled rendering
mode (`0 Tr`) in otherwise supported source content. Preserve both default and
nondefault values, including inherited state before a form invocation and changes
between text objects. Keep q/Q and form-call scoping correct: BT does not reset
these graphics-state text parameters; form invocation saves/restores them.

The existing Edit text workflow lists and edits supported occurrences. No new
spacing or transform controls are added. Same-font replacement retains the
original character codes/font and state. Added or removed characters use that
state naturally; other occurrences keep their original pixels and glyph geometry.
Source files, other pages, undo/redo, source ownership, protection and recovery
continue to use the existing immutable source replacement path.

Retain the current single Tj per text object, explicit local Tf/position, Latin
font proofs, bounded affine matrices, ancestor clipping and collision checks.
Do not infer paragraph groups, implement TJ adjustments/quote operators, enable
stroke/clip/invisible rendering modes, or relax tagged/layered/transparency/image
restrictions in this change. Those remain separate compatibility work.

## Admission and state model

Track a small Copy/PartialEq text-state value on each admitted occurrence, with
PDF defaults: character/word spacing 0, horizontal scale 100, rise/leading 0,
filled rendering mode 0. Pass inherited state into recursive flatten validation;
q/Q saves/restores state along with the already tracked transform. A child form
receives a copy and cannot leak its local changes into the caller.

Validate exact operand counts and finite numeric operands before recording them.
Use the existing bounded-number admission (absolute magnitude <=1,000,000).
For Tz additionally require 0.01 <= abs(value) <=10,000; allow negative values as
reflection, subject to the existing final nonsingular/magnitude matrix and glyph
checks. Tr requires the integer zero. All remaining native limits stay in force.
Keep the original operations in both the private validation stream and published
resource chain. Compare effective state for corresponding occurrences before and
after publication, in addition to existing font/style/placement checks.

## Explicit font substitution

Character spacing, scale and rise are independent of the chosen character-code
width and must be retained when substituting a font. Word spacing is different:
PDF applies Tw to single-byte code 32, not to a two-byte 0020 code. Folio installs
substitute fonts with Identity-H encoding. Do not silently lose effective word
spacing when changing a simple font to that encoding.

For this milestone, an occurrence with nonzero effective Tw and a single-byte
source font remains editable using its original font but cannot install a new
substitute font. Set its existing canSubstitute capability false and enforce the
same rule natively, with an explanation that its original word spacing cannot
be preserved by that font change. Other occurrences on the page remain eligible.
Original Identity-H fonts have no single-byte codes, so nonzero Tw is inert and
must not unnecessarily block their substitution. Reuse of the exact verified
original program/encoding is a no-change font request and may remain allowed.

This is an explicit compatibility limit, not a claim of complete typography
support. A later encoding-aware TJ writer can remove it with its own proofs.

Reference: [ISO 32000-1, section 9.3](https://developer.adobe.com/document-services/docs/assets/35e4369068f86065372c18787171a17e/PDF_ISO_32000-1.pdf).

## Validation and preservation contract

Before publishing, retain the existing source-page copy comparison, actual no-op
occurrence clone comparison, original/private-flat glyph and raster equality,
verified source encoding, exact replacement extraction, other-run identity, bounds
in every ancestor's own coordinates, collision checks, and unchanged pixels outside
old/new edited bounds. Do not weaken comparisons to make a fixture pass. If native
extraction introduces apparent whitespace, investigate actual glyph/code mapping
rather than accepting ambiguous text.

Generated fixtures must exercise:

- All default operators and nonzero Tc/Tw/Ts/TL, positive/negative Tz; standard,
  simple embedded and Identity-H fonts, with changed and exact no-op replacements.
- State before BT, between BT/ET objects, in page/wrapper/leaf scopes, nested q/Q,
  sibling forms, repeated shared placements and a second page.
- Page rotations and representative affine transforms, nonzero CropBox and decimal
  UserUnit. Verify untouched placement pixels and source hashes independently.
- Font substitution with preserved Tc/Tz/Ts, rejected single-byte nonzero Tw,
  allowed inert Identity-H Tw and zero-Tw overrides. A sibling's Tw must not
  disable the selected occurrence's font chooser.
- Malformed operands, nonfinite/oversized state, zero/out-of-range scale, non-fill
  rendering modes, clipped/overlapping replacements and stale paths/text.

Use red/green native tests, then existing form/text/font/protection regressions.
Capture saved outputs and compare with separately authored reference PDFs using
MuPDF/pypdf, including unchanged outside-region pixels and other-page content.
Run frontend tests only if UI behavior changes, or the affected dialog tests for
native capability handling. Build a fresh fingerprinted executable; on Windows,
edit a generated nested-form document, undo/redo, save/reopen and verify output.
Independent implementation review remains required. Do not claim hardware,
full-corpus, unsupported-script, all-font, or Adobe/Foxit parity acceptance.

## Decisions left to implementation

Choose internal helper names, fixture factorization and the smallest deterministic
matrix that covers each state/scoping dimension without a redundant cross-product.
Add frontend explanatory copy only if the current canSubstitute handling does not
already communicate the native limitation. Never change a rejected draft or choose
a replacement font automatically.
