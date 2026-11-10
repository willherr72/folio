# Faithful content-stream number correspondence

Continue the private JPEG placement phase without enabling JPEG admission.
Prior evidence shows original raw numeric operands cannot be equated with
lopdf's binary32 value solely from valid PDF numeric grammar.

Check raw content numeric tokens before allocating the content decoder. Keep
the current single-sign, at-most-one-dot grammar and 64-byte token limit.
Integer magnitudes remain bounded by the existing one-million policy. For
decimals, establish that PDFium and MuPDF obtain the same finite binary32 value
as Rust/lopdf; also check the exact decimal spelling emitted by the current
lopdf writer. Refuse ambiguity before mutation. No numeric-token normalization
may silently redefine the source.

Independently implement a bounded compatibility checker, not a copy of MuPDF
source. Its normal-decimal proof uses the reader's first nine significant
digits and decimal exponent. For exponents -13 through13, conversion is
correctly rounded. For smaller exponents, enclose the intermediate using the
32-significand-bit cached-power relative error <=2^-32 and round-to-odd relative
error <2^-31: q*(1-v)/(1+u) through q*(1+v)/(1-u), with outward binary64 math.
Both interval endpoints must round to the same nonzero target binary32. Native
underflow-to-one cannot pass. Prove the pinned cached-power table bound using
exact rational audit arithmetic; do not copy its table into product code.

For the long-integer-spelling alternate path, independently evaluate positive
decimal digit accumulation with binary32 endpoint intervals. Enclose both
fused and unfused multiply/add, use monotonic division/addition, and compare
both resulting endpoints to the target. Permit infinity only for a denominator
with finite numerator; refuse nonfinite output and indeterminate infinity over
infinity. Leading zeros with ordinary values remain supported.

Integrate with preflight, and recheck the final private content serialization
before publication. This checks newly emitted operands as well as source
operands. Preserve current output bytes for admitted inputs. Existing operation
and token budgets still apply. Text strings, names, comments and hexadecimal
strings are not numeric operands and must be skipped correctly.

Verify with observed red/green cases for long spelling, native underflow-to-one,
nine-digit truncation around halfway, signs, zero, small values, and malformed
tokens. Export native Rust acceptance decisions and canonical spellings for a
deterministic boundary/random corpus. Audit accepted original and serialized
values through actual PDFium parsed image matrices and MuPDF's PDF lexer.
Record refused native disagreements and proof conservatism separately.

Scope: content stream operands only. Dictionary numeric spellings and complete
renderer placement (norms, grid fit, inverse/affine paths, opaque paint) remain
unproved. This check is not final JPEG disclosure acceptance. One final fresh
phase3 review remains after integrated implementation, not per helper.
