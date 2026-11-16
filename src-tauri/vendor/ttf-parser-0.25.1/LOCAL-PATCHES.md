# Folio local CFF proof extensions

Based on ttf-parser0.25.1, upstream commit
`56c33b910b03ca152f78363ec471c5dfd97c3069`. Upstream MIT and Apache2 licenses
remain in this directory. The original source map is retained in the component's
`ttf-parser-upstream.json` evidence.

FDSelect is validated against the actual glyph and Font DICT counts: first range
starts at0, starts increase strictly, the final sentinel equals glyph count, and
every selected dictionary exists. This rejects malformed ownership tables before
they can authorize an edit. Existing valid SID/CID parsing and integral public
width behavior remain unchanged.

`glyph_infos` traverses charset ranges sequentially, detects repeated identifiers,
reports errors separately from exhaustion and accounts for actual Font DICT
matrices. FDSelect lookup uses validated ranges and binary search. Fixed-size
identifier/matrix state avoids per-glyph allocation or repeated range scans.

`glyph_width_with_budget` retains fractional SID/CID widths, selects the actual
Private DICT and charges repeated Type2 interpretation/subroutine work against a
shared caller allowance. Dictionary bytes and variable glyph-name strings are
charged before inspection. `glyph_index_exact` exposes the declared built-in
encoding without the convenience API's Standard fallback. Existing integral
width and fallback APIs retain their contracts.

The budgeted source-proof width path continues through the complete glyph
charstring and reachable subroutine interpretation after an explicit width.
It requires endchar and charges the tail to the same caller allowance.
Budgeted proofs also require return/endchar for reachable subroutines and reject
residual endchar operands that cannot be a legal width or seac argument list.
Legitimate zero-ink space glyphs may return a zero rectangle after validation;
the upstream convenience width/outline APIs retain their existing behavior.

`parse_for_proof` is a separate strict entry point for one CFF1 source font.
It checks complete metric dictionaries, finite operands/arity, duplicate
declarations, index entries, private subroutine placement and conflicting custom
encoding entries with the same caller budget. Generic rendering parsers retain
their permissive behavior. `predefined_encoding` identifies Standard/Expert
without substitution; exact lookup uses Expert SIDs rather than Standard's
uppercase slots. The old convenience lookup keeps its upstream contract.

Folio's source-resource integration consumes these primitives separately from
its TrueType path. Focused native edit/save/reopen checks exist; complete
application and distribution acceptance remain governed by the component spec
and plan dated2026-11-12.

`encoding_glyphs_with_budget` resolves all single-byte entries with one charset
pass and at most256 sorted SID requests. It preserves omitted codes, rejects
duplicate identifiers/competing codes and missing supplements, and charges the
scan, comparisons and fixed-size sort against the caller budget. This avoids
one full-font scan per byte when a simple resource embeds a large program.
Exact single-code lookup also resolves predefined ISOAdobe/Expert charsets;
the original convenience lookup remains unchanged.
