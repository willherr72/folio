# Exact source text in editing snapshots

The dense-page desktop reproduction contains `(Old) Tj`, but PDFium's object text
API returns `Old ` by including an unowned generated separator. Exact character
ownership returns O/l/d; the next page character is a generated, unowned space.
Both legacy and complete-source edits then write that separator during the no-op
proof and refuse an otherwise valid edit. Merely trimming strings would lose real
spaces and weaken the source contract.

The expanded probe also showed that native owned-character extraction collapses
two explicit source spaces into one. Ruling: use verified PDF operand text rather
than an owner-only normalizer. Preserve raw RunSnapshot text, paragraph anchors,
selection identity and all full-page glyph checks unchanged.

A bounded source-text helper maps top-level show operators to native runs by
ordinal, verifies active font name/size, decodes using the verified font encoding,
requires an exact byte roundtrip and identical non-space characters in order, and
validates positioned-array character boundaries. Actual spaces come from operands.
An optional sourceText field is emitted only when it differs from native text;
the dialog edits that value, while App still sends native text for stale identity.
The engine independently recomputes source text for no-op, positioned replacement,
substitution normalization and exact changed-text verification. Client sourceText
is never an authorization input. Failed mapping retains existing behavior.

Bounds remain16MiB decoded content,100000 operations,64 resource/state levels and
existing font/string limits. No spatial matching or ownership-key guessing is used.

Alternatives considered: strip trailing whitespace (corrupts actual text); normalize
only mutation requests (listing, paragraph anchors and positioned operands disagree);
or change PDFium (larger renderer-wide behavioral/packaging change). The verified
operand path is selected under the standing autonomous-work authorization.

Scope includes single-run listing, no-op, replacement and substitution. Keep
RunSnapshot and full-page glyph_snapshot unchanged:
no-op/publication still compares all physical and generated glyphs and exact pixels.
Font/placement/collision/source-byte authorization and stale text checks remain.
No saved metadata or command change; listing gains optional sourceText. Existing
paragraph replay must be tested. Native-only omission of actual non-space glyphs
and unsupported font encodings remain refusals.

Acceptance: original failing dense edit, actual whitespace, colliding owner keys,
generated internal gaps, embedded fonts and rotated text, existing native edit/
group/paragraph/form suites, dense timing, Windows edit/save/undo and independent
authored references. Preserve failed evidence and do not accept the pending build
until the real desktop reproduction passes. Broad parity remains unproven.
