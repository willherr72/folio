# Source-bound accent clusters

Folio currently refuses a PDF whose ToUnicode maps one visible precomposed glyph to a base letter followed by combining accents. A public DejaVu witness at `artifacts/source-accent-clusters-20261115/probe-1` reproduces this: both independent readers extract `Cafe\u0301`, while the current compiled CLI refuses editing.

Use the existing verified physical source-glyph codebook for these clusters. Keep source and replacement Unicode exact; normalization supplies alias comparison evidence only. No general shaping, standalone mark positioning, or imported-font policy relaxation is authorized by this change.

## Approach and boundaries

Chosen implementation: admit a bounded source sequence consisting of one already-allowed alphabetic starter and 1–15 assigned U+0300–036F accents with nonzero canonical combining class. The entire sequence must also pass existing actual code/GID, intrinsic alias, positive outline, exact width, resource identity and original operand proof. Retain existing Latin multi-letter ligatures. Complete form-font repertoire proof remains scalar-only.

Changing global `allowed_character`, normalizing document text, or laying out accents as separate scalar glyphs would break fidelity and is excluded. Existing atom-aware styled paragraph layout is the paragraph route; legacy scalar paragraph layout may continue to refuse these clusters safely.

Use a proof-aware source-text predicate only for verified original-font routes. Explicit replacement-font requests keep imported-font validation. Source-bound single/grouped/shared-form editing and modern styled paragraph reflow must preserve physical atoms, exact logical Unicode, surrounding text and save/reopen behavior. Unavailable accents, split atoms, contradictory aliases and malformed mappings remain refusals.

## Evidence

Generate renamed, licensed witnesses from the existing public DejaVu font: CID TrueType and six supported CFF1 representations. Include Latin acute, Greek acute, Cyrillic breve and two accents in one physical glyph. Verify real native editing, no-op pixels, save/reopen/re-edit and styled paragraph memory. Independently inspect actual saved PDF Unicode, physical source codes, font program and f32 widths, and render expected source-authored output. Run complete native library correctness, affected optimized integration targets, fresh frontend/build and actual compiled workflows before closing. One final scoped review, one TDD correction pass; no package promotion or Git mutation.

## Reviewed correction

The final review identified a compatible alias hiding contradictory intrinsic aliases and a stale desktop fixture name. The correction tracks one bounded comparison normalization per candidate GID and rejects contrary aliases or cross-subtable disagreement even when another alias matches. Absent intrinsic evidence remains distinct from contradictory evidence. Four equivalent-alias and four contrary-alias public font witnesses exercise both outcomes; no source/replacement string is normalized. Desktop choices are validated against the authority before creating a runtime attempt. Both findings have RED→GREEN evidence; scoped native/frontend/build/actual-runtime qualification is complete in the bound completion record. Broad Folio quality and release qualification remain open.
