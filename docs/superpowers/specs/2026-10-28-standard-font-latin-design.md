# Source-bound standard-font Latin editing

## Problem and outcome

The accepted 0.23 package still rejects original ReportLab and PyMuPDF Helvetica
lines containing `Résumé café`. Both use ordinary Type1 WinAnsi resources without
width or Unicode overrides. Fresh responses and unchanged-source hashes are in
`artifacts/producer-refresh-20261028/standard-latin-refusals.json`.

Make representable Latin text and punctuation editable with the original standard
font through existing single-run and paragraph workflows, including saved re-editing
and explicit font substitution. The same native implementation serves desktop and
CLI/MCP. Retain glyph placement, following text, neighboring content and original
document graph. This is an editing improvement, not a claim of general Unicode
shaping or arbitrary custom-font support.

## Chosen approach

Coordinator choice: add a bounded standard-font proof bound to the actual source
resource for each text-show operation. Reuse named encoding tables and licensed
standard-font advances already in the repository. Keep embedded-font proofs
separate: a standard font has no verified TrueType program/glyph identity.

Changing only the ASCII predicate would misdecode WinAnsi bytes as UTF-8 and leave
paragraph advances inconsistent. Automatically substituting another font would
change appearance without the user's explicit choice. Neither satisfies the goal.

`standard_font.rs` owns `StandardFont` and checked encode/decode/advance methods.
`source_text.rs` owns the shared bounded operation/resource traversal and binds a
proof to the corresponding `RunSnapshot`, matching font name, size, operation
ordinal and decoded native text. Do not associate proofs solely by BaseFont name:
two resources may have the same name and different encodings. Add an optional
standard proof to the private snapshot; its Unicode text and geometry still come
from the original native snapshot until existing source-space reconciliation.

Admit the twelve Latin standard faces (Helvetica, Times and Courier variants)
with proved StandardEncoding, WinAnsiEncoding or MacRomanEncoding resources.
Preserve the existing equivalent-encoding dictionary policy. Decode only defined
codes and encode only representable supported Latin characters. Handle aliases
explicitly: do not silently map an encoded space or glyph to a different code;
source operand decode/re-encode must be exact. Undefined codes, incomplete or
contradictory mappings and unsupported glyphs remain refusals. StandardEncoding
does not contain every WinAnsi accent; explicit substitution remains the route
for characters absent from the original resource.

Only nominal standard resources get the new proof. Width overrides, ToUnicode
overrides and descriptors that change nominal glyph behavior require their own
proof and cannot inherit this authority. Keep existing successful ASCII behavior
for resources outside the new proof path, with its existing preservation guards.
Do not treat a missing new proof as permission to emit non-ASCII UTF-8 bytes.

Keep 4,096 UTF-8-byte text limits; bound encoded operands before decoding. Retain
the existing 16 MiB content, 100,000 operation and 64-level resource/state limits.
Do not retain whole additional PDF graphs merely to cache a font mapping.

## Integration and invariants

All checked text encoding/decoding and source advances consume the same proof.
That includes original-font positioned `Tj`, `TJ`, quote operators, substitution
cursor correction, uniform/styled paragraph layout, fragment replay and grouping.
Paragraph style equality includes the encoding/metric proof. Form validation
views bind their isolated flattened resources before decoding source operands;
this must not accidentally broaden unproved form resource admission.

Preserve stream operators where required; do not route newly admitted text through
a serializer that normalizes the original resource mapping. Keep exact native
no-op text/style/glyph/raster checks, bounded collision checks, complete-source
publication and independent output audits. Failed edits produce no published
candidate and leave the original untouched. Substitution remains explicit.

## Acceptance

- Failing-then-passing native public-API tests for original-font accented text
  insertion, exact no-op appearance, paragraph reflow and saved re-editing.
- All twelve Latin faces; WinAnsi and MacRoman accents; representable Standard
  punctuation; source-equivalent indirect encoding dictionaries; non-ASCII TJ
  segment boundaries and dependent following text; quarter turns and physical
  units in representative combined cases.
- Same-name differently encoded resources do not share proofs. Wrong/undefined
  codes, altered mappings, nominal-width overrides and glyph-affecting descriptors
  do not gain authority. Existing embedded and ASCII cases remain green.
- Two untouched producer originals pass original-font single-run and paragraph
  edits through CLI and Windows, then saved-file re-editing. Explicit-font and MCP
  coverage exercises the shared implementation. MuPDF/pypdf text, original font
  resources, nonselected glyphs/pixels and second pages agree; negative controls
  demonstrate that output corruption is caught.
- Build the same final fingerprint used for regression/acceptance; one final
  scoped review and a durable checkpoint with remaining limits.

## Operating constraints

Continue inline in the existing linked worktree. No routine design/permission
questions or per-task delegates: the user's standing autonomy preference applies.
One final reviewer. No commits, publication, package overwrite or cleanup. Preserve
all originals, prior packages and failed evidence. One Cargo process, locked/offline,
jobs1, serial native tests, established shared target. Freeze product/test inputs
during each gate. Do not alter the accepted 0.23 package. The broad goal stays active.
