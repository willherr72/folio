# Positive choice font repertoire proof

Execute inline with TDD. Current checkpoint47812114...08101 has verified empty
transitions. Preserve inputs/evidence, no commits or concurrent Cargo/builds.

## Design

Add a private ChoiceFontProof bound to one exact PDF font resource. Validate all
required label strings before allocating font/proof data: max4001strings,
4096UTF8bytes each and128KiB combined. Provide checked supports/encode/decode so
native generated character codes can later be checked against the same resource.
This is repertoire/code mapping evidence, not shaping or native-encoder proof.

For embedded TrueType/Identity-H resources, reuse the existing positive verifier
of PDF Unicode/code/glyph/width agreement. Keep the existing Latin text-editing
entry point unchanged. Add a separate Unicode-resource entry point that applies
the same agreement checks to all represented scalars, including agreement among
Unicode cmap subtables. Bound cmap work through the existing font-registry guard.
Use an indexed code-to-Unicode decoder; never scan the whole font per character.
Do not widen support to missing glyphs, ambiguous maps, substituted programs,
unverified widths, vertical/CFF/variable fonts or ligature mappings.

For standard nonembedded Latin base fonts, accept only explicit supported base
encodings (or the defined standard default), and verify every text round trip.
Reject unknown/Differences/ToUnicode overrides rather than let lopdf's lenient
font-encoding fallback authorize them. An embedded program or symbolic descriptor
must not be treated as standard merely because its BaseFont name is familiar.

The later publication layer must bind these proofs to actual widget fonts, cover
unselected/offscreen labels, compare native generated codes/text, and separately
prove layout and glyph rendering. A positive Unicode repertoire does not establish
surrogate handling, shaping or bidirectional layout in the native form generator.

## Task1 — proof and codec

Files: new choice_font.rs and choice_font_tests.rs; lib.rs wiring; narrow internal
reexports in engine.rs/text_edit.rs; embedded_font.rs Unicode wrapper/shared core;
fonts.rs visibility for bounded_cmap_coverage only.
API: ChoiceFontProof::for_texts(document,font_object,texts), supports(texts),
encode(text),decode(bytes). All crate-private, no user-supplied font identity.

- [x] Baseline and missing-API RED: standard WinAnsi accents, unknown/custom
  encoding refusal, full embedded WinAnsi, Identity-H Greek beyond Latin, missing
  glyph/mismatched glyph-width refusal, code round trips and bounded inputs.
- [x] Implement shared positive proof with unchanged old Latin policy, indexed
  decoder and standard encoding admission without fallback.
- [x] Run focused plus affected native font/choice/text regressions; record exact
  evidence and remaining native-codec/publication/UI requirements.

Review focus: resource identity, not program name alone; conflicting cmaps;
source glyph0; malicious work amplification; repeated decoder lookup; scalar/code
length agreement; and preserving old text-editing policy while proving broader
font repertoire for choices. Whole-feature review remains pending.

Rulings recorded in the execution ledger: initially admit StandardEncoding and
WinAnsiEncoding for nonembedded base fonts; other encodings need positive glyph
evidence. Correct the shared verifier's nonfinite/malformed DW fallback (observed
RED then GREEN), without widening the old Latin editing policy. Added a Greek
subtable-conflict fixture, strict decoder/output budgets and indirect/default
standard encoding checks. Eight focused tests passed before expanded final gate.

Final gate passed369 tests,3ignored across library and seven integration targets.
Source fingerprintc08cb3868b24d53314ddf7f895f44a30a79d5651af0709ef469d7bade5193c70.
Evidence: artifacts/form-choice-font-proof-20261025/verification.json. This remains
a private repertoire/codec layer; next plan is form-choice-label-admission.
