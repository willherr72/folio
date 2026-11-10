# Native choice text encoding and fidelity

Continue the complete existing-choice editor inline. Starting checkpoint:
b291446103a260678302eacf3efcbb7da61d68bb3559003b9c13f6b6bff759ee,
339native tests and19independent cases passed. No active Cargo/build/app.
Preserve inputs/failed evidence; one Cargo job; no commits. Final whole-feature
review remains after public integration, not per private task.

## Design and source findings

NativeForm::open uses FORM_OnAfterLoadPage and the SDK widget appearance path:
fpdfsdk/cpdfsdk_appstream.cpp SetAsComboBox/SetAsListBox, not the similarly named
core GenerateAP functions. SDK lists write every row starting at TI (with clipping);
dropdowns write only selected label or GetValue when no row is selected. GetValue
falls back to raw DV only if V is absent; a matched export default is then visible
as raw text, not automatically as its display label. Preserve that distinction.

Both SDK paths use CPWL_EditImpl/GetEditAppStream and uint16 CPVT words. Nominal
font repertoire is insufficient to prove native surrogate/character handling.
Do not reject all supplementary characters just to avoid this check: first prove
their actual resource mapping, then observe native encoding and either preserve
their text or return a precise unsupported-generation result.

Prerequisite discovered while building supplementary coverage: the new Unicode
font proof compares BMP-only cmap subtables against supplementary code points.
A BMP subtable cannot represent them and its absence is not a conflict. Exclude
only maps whose format cannot represent the scalar; retain agreement among every
capable Unicode map and all existing code/glyph/width checks. Old Latin policy
must stay unchanged. Prove this correction before relying on the new fixture.

## Native audit architecture

Add private codec list widgets alongside the real widget in its existing one-page
probe. They use the exact copied font resource (one shared DR graph), fixed12pt,
no actions, empty selection and all required strings as display labels. Use SDK
list generation because it emits each supplied row. Batch by128rows and4096UTF8
bytes, at most128batches for the current bounded payload; never duplicate font
graphs across batches. Fixed wide rectangles prevent incidental horizontal fit
from controlling the codec check; they are not published or rasterized.

Validate each generated batch's bounded AP/resources against the original font.
Decode Tj operands with ChoiceFontProof, retaining BT/ET row boundaries, and compare
each complete row exactly to its required string. Charge decoded bytes before
append; reject missing/extra rows, incomplete/unknown codes, fallback resources,
unbound fonts or unsupported text operators. Existing256KiB/16384-op AP limits
apply per bounded batch; total probe32MiB still applies. Empty rows remain explicit.

Validate real appearance text too: dropdown expected text includes raw absent-V
fallback; lists expect labels from TI through end. Use exact row grouping rather
than a substring or set comparison. Geometry/ink/bidi placement still needs its
separate publication proof; exact decoded code order is not that proof.

After proof, remove codec widget references from private page/AcroForm and prune
unreachable objects before returning the main generated appearance/document. No
private audit fields may become reachable in a candidate, even accidentally.

## Task1 — supplementary repertoire prerequisite

- [ ] Archive affected source/test baseline. Add a true UTF-16 surrogate ToUnicode
  fixture for DejaVuSerif U+1D538. Observe current incorrect conflict RED.
- [ ] Compare only format-capable Unicode subtables in the new proof path; keep
  old Latin verifier unchanged. Prove exact two-byte CID encode/decode and retain
  existing Greek-conflict/glyph/width regressions.

## Task2 — native codec implementation

Prerequisite discovered by the native Greek test: SDK generation substitutes Arial
for the real embedded DejaVuSerif Omega despite a verified mapping. Captured input/
output proves both original program retention and actual Tf/_A1 fallback. The
renderer tests the requested charset before considering its original embedded font.
Add a pinned renderer correction before judging codec support: prefer the original
embedded horizontal face when its Unicode/code round trip and actual nonzero,
in-range glyph prove the requested BMP word. Retain existing fallback behavior for
missing mappings/glyphs, out-of-range glyphs, nonembedded and vertical fonts.

Build a separate renderer candidate with provenance, preserving the current DLL.
Direct DLL regression must cover combo/list/text fields, Greek/mixed text and
negative glyph/fallback controls, plus all existing renderer probes. Update recipe,
setup tamper tests and source fingerprints coherently; never silently replace the
renderer with an unverified artifact. Native/desktop acceptance must expand for a
renderer change. Do not treat this dependency correction as choice-editor completion.

- [ ] RED native hidden supplementary/altered-string regressions plus bounded
  grouping/decoding tests. A supported nominal map must not hide native text loss.
- [ ] Implement bounded borrowed batches, shared-resource audit widgets, exact
  generated text checks and audit graph removal in form_choice_probe.rs (factor
  into private choice_codec module as useful). Include actual fallback text in
  required strings and actual appearance expectations.
- [ ] Cover Greek/accents, spaces/empty labels, duplicate labels/export keys,
  current/default fallback distinctions, top index, linked resources, operation/
  aggregate bounds, and no source mutation. Exercise max-row/batch work with
  meaningful bounded performance evidence. Test original font programs/resources
  remain shared rather than multiplied.

## Task3 — verification and record

- [ ] Run focused and affected library/choice/font gates on stable source, then
  independent raw-code/font/pixel checks and hash verification. Preserve failures.
- [ ] Record exact acceptance limits. Continue geometry/source no-op/global flags,
  atomic publication, worker permissions, UI/history/save/recovery, final review
  and a fresh Windows build. This task alone is not the complete choice editor.
