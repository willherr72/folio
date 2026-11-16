# Source-bound ligature editing

Public reproduction: artifacts/ligature-source-editing-20261113/reproduction.json. The same logical `Office` is editable when encoded as six characters, but not with one `fi` or `ffi` glyph. Both original-font editing and explicit substitution are refused. The source files use the existing public DejaVu font and retain ordinary scalar glyphs; this is a source binding defect, not a missing-glyph fixture.

## Required behavior

Existing PDF codes map to **glyph atoms**, each containing one or more Unicode scalars, one glyph ID and one advance. Unicode text is the editor representation. Source binding must retain the actual code sequence independently of that representation. An unchanged edit must retain exact bytes, not reverse-encode the letters and silently decompose a ligature. Changed text may reuse complete proved atoms; an edit inside an atom needs separately proved replacement glyphs, or the existing explicit substitute-font route.

Preserve complete unchanged atoms throughout a replacement, including separated edits, by a deterministic bounded alignment. Never split a glyph code. Character spacing counts encoded glyphs, not extracted Unicode scalars. TJ boundaries must be glyph boundaries; substitution must translate logical boundaries without placing an adjustment inside a new glyph. Paragraph metrics and wrapping must use the same atom segmentation as publication, including repeated letters having different glyph advances in a single style. No global per-letter metric approximation.

## Mapping proof

Retain the existing structural CMap parser, codespaces, unique-code checks and limits. Parse bounded valid multi-scalar UTF-16 destinations separately from the scalar repertoire. Start with unshaped Latin sequences; controls, whitespace, combining/shaping/bidi text and malformed destinations remain unavailable. Each admitted sequence needs the same actual CID-to-GID, outline, advance and PDF-width checks as a scalar. Complete form-font proof, simple WinAnsi, legacy encodings and CFF paths must retain their current strict admission until their own exact proof exists. No system-font inference, repaired mappings or synthesized outlines.

Intrinsic program aliases also remain evidence: an ordinary l glyph cannot become fi merely by relabeling ToUnicode. For sequence-bearing fonts, bounded Unicode cmap enumeration compares public assigned glyph aliases using compatibility decomposition, solely as comparison evidence. Matching ﬁ/ﬃ aliases support the source's already-proved glyph. An unavailable/budget-exceeded scan supplies no new sequence authority. A genuinely absent cmap or a CID glyph without any meaningful Unicode alias retains the established exact CID proof; do not require a presentation-form mapping for every valid subset. An unusable sequence is excluded without revoking existing proved scalar codes.

## Bounded atom model

Use a small dependency-free Rust module that can also be tested directly by rustc. It accepts already verified symbols; it is not itself a font verifier. Immutable codebook owns symbol identity and encoded width. Binding decodes at most 8192 bytes to at most 4096 UTF-8 bytes; code length is one or two. Each symbol has at most 16 scalars / 64 UTF-8 bytes; at most 512 multi-scalar symbols and 32 KiB of their text. Total codes at most 65536. Duplicate codes and duplicate text mappings are rejected rather than guessed. Widths must be finite within the existing 0..10000 nominal bounds and GID must be nonzero.

Replacement alignment matches whole source atoms against replacement scalars. Maximize retained Unicode length subject to complete gap encoding, resolve ties deterministically, and retain exact source codes and advances. Encoding transitions participate in alignment: unconstrained text matching can strand an unsupported gap. Prefix/suffix matching skips the table only when it proves that ALL source atoms can be retained with an encodable insertion. Greedy prefix consumption is otherwise unsafe (source atoms `a` + `ab`, replacement `ab`). The dynamic programming table is bounded by 4097 squared u16 cells (under 34 MiB transient); no recursive search. Encode gaps using only proved symbols, preferring single scalars then the longest sequence that permits complete encoding. Budget checks precede allocation/copy. No-op binding, width, glyph count and byte boundaries come from the same encoded representation. Failed replacements leave the source unchanged.

## Integration and acceptance

1. Atom model and adversarial/unit oracle coverage. This alone does not enable ligatures in Folio.
2. CMap sequence evidence and exact source binding, original-font/explicit-font single-run and dependent-cursor edits, TJ spacing. Preserve scalar-only behavior.
3. Per-occurrence paragraph atom metrics/layout/publication/replay; no-op, growth and repeated edit proof.
4. Affected native and frontend contracts, current compiled CLI and owned Windows workflows, independently extracted/raster-checked saved output and font-resource preservation, refusal controls, one final fresh review and disposition.

All failures and original files remain. Selected 0.50 and reviewed separate 0.51 remain unchanged. No clipboard, private QA reuse, portable selection, commits, merge or publish. Parent source 535687099d0448ee8d3e287b1fd0774e462b6b6d943b43b6f052b9a16b62db1b and parent checkpoint remain authorities for the completed CID component. No ligature-support claim until all integration gates pass; broad goal remains active.
