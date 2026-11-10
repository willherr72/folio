# Saved form appearance/value consistency

The previous goal turn made progress: a verified appearance-control checkpoint was
built and tested. The current public regression reproduces an additional loss:
changing only size replaces AP text `Visible answer` with stored V `Original value`.
Standing user authorization permits fixing it inline without another approval gate.

Appearance editing must preserve the stored value and must not silently replace a
different saved answer. Deriving a new V from the appearance would change linked
values and semantics; rebuilding from V without checking the AP already loses text.
Use a bounded original-font AP probe to establish consistency before editing.
If the saved answer differs or cannot be read unambiguously, refuse with a useful
reason; normal form filling remains the explicit value-editing workflow.

Copy only the admitted normal appearance/resource graph into a private one-page
document using the existing bounded appearance Copier. Inspect its native Unicode
characters, including generated-separator flags. Compare non-whitespace characters
and whitespace boundaries to the inherited PDF text-string value. Real whitespace
requires a value boundary; generated layout separators allow a boundary or none,
so wrapping and comb spacing do not invent content. Do not globally remove spaces:
`A B` versus `AB` without a generated boundary must remain distinguishable.
Leading/trailing whitespace may collapse because it has no painted glyph; preserve
the original V byte-for-byte through the existing immutable transaction. No Unicode
normalization, case folding or replacement-character inference. Bound value/probe
character and retained Unicode sizes before iteration/allocation; copying remains
under existing graph/font budgets. Missing AP is supported only for empty values.

Reuse the already parsed source document. Native inspection and final generated
appearance validation both perform this check; mutation still checks source identity,
expected value/bounds, font graph and permissions. No new frontend response fields,
silent substitution, source changes, or text repair. Existing clear-error rendering
shows the refusal. The broader Adobe/Foxit objective remains active.

Acceptance includes differing answers, extra decorative text, empty stale text,
space-boundary mismatch, matching embedded/standard values, accents, multiline
wrapping, comb-generated separators and save→reopen→later filling. Verify source
integrity, stable font/value/geometry, existing45 native cases, full frontend and a
new desktop build. Use one final scoped reviewer, then parent fixes without rereview.
Retain previous binaries, baselines, failed evidence and exact evidence hashes.
