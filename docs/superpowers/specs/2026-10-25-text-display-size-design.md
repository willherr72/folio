# Physical size labels for existing PDF text

The original quiz's highlighted paragraph now previews an insertion, but its
single-run dialog reports 1 pt for 12 pt text. The stored Tf operand is 1 and the
text transform scales it by 12. Form and grouped labels use the same raw value.

Add an optional displayFontSize measurement to native text-run, reusable-form
run and grouped-preview DTOs. Preserve existing fontSize and every transaction
identity/operand. Compute the vertical em vector length: abs(Tf) * hypot(c,d)
from the complete source text transform, then apply UserUnit exactly once in the
existing physical-unit adapter. Rotation/reflection do not change that length;
nonuniform scaling/skew report the vertical em-vector measurement, not glyph ink
height or inferred typographic intent. Omit an unrepresentable/nonpositive value.

Approaches: changing fontSize in place would silently redefine a public field;
frontend geometry inference cannot recover the original font transform reliably.
Chosen additive measurement preserves old API data and old payload compatibility.
The existing/group dialogs prefer the measured value and fall back to fontSize
when absent. PDF mutation paths, raw snapshots, paragraph sizing and rendering
remain unchanged. FontPicker sizing is not being redefined in this correction.

Prove scaled, rotated, reflected and nonuniform direct runs; nested form transforms;
group inspection; UserUnit; exact no-op pixels and original source preservation.
Frontend tests prove the visible label while submitted edit data remains intact.
Desktop checks use a synthetic fixture and the unchanged original quiz locally.
Private document text/screenshots remain under artifacts/quiz-current-20261025/private.
No release/parity claim, commits, or user-document overwrites.
