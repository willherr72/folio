# Browser-produced PDF compatibility and optimized timing

The representative corpus currently covers Word, ReportLab and PyMuPDF. Extend it
with original Edge/Skia PDF output from an owned, network-free HTML print fixture.
Generate both tagged and untagged originals with the same two-page content: a body
paragraph, table row, bordered callout, web link and unchanged reference page.
Do not normalize or repair originals before inspecting/editing them.

Use the accepted optimized0.29CLI, whose native engine/dependencies are identical
to current source. Capture per-command wall time including process startup, native
validation/rendering and file work; this is not desktop Apply latency. Exercise
body/table/callout text and paragraph edits, two saved edits per case. Preserve
all refusal details. Successful native calls require independent complete-text,
fonts/style, outside-region pixel/glyph, links and untouched-page checks. Region
selection must not allow changed output to enlarge its own exclusion mask.

Historical optimized0.27calls suggest subsecond previews, so a speculative speed
patch is unwarranted. Measure first; investigate demonstrated current failures
with a failing regression before product changes. A finite authored browser corpus
is stronger producer coverage, not arbitrary-web-document/IME/parity certification.
No browser network, user files, original overwrites, dependencies or portable
release changes. Autonomous authorization and existing worktree apply.

## Evidence-driven correction: table relationship attributes
Original tagged Edge cell MCID11 belongs to NonStruct30, under TD29. TD29 has
Table-owner Headers/RowSpan/ColSpan attributes; the current validator forbids all
/A entries on ancestors. Untagged identical text edits; tagged body/callout edits.
Permit only structurally unchanged Table attributes on literal TH/TD roles:
positive integer RowSpan/ColSpan, Headers arrays of bounded nonempty byte strings,
and Scope Row/Column/Both on TH. Accept dictionary or bounded array of dictionaries
(including bounded indirect references), no revision entries in this first scope.
Unknown owners/keys, summaries, layout geometry, classes, alternate/actual text,
malformed values, cyclic references and ownership mismatches remain refused.
Do not rewrite/repair attributes, tree, IDs or header relationships. This is
verbatim preservation, not table remediation/validity certification.
Source: ISO32000-1 table349, Adobe published PDF standard:
https://opensource.adobe.com/dc-acrobat-sdk-docs/standards/pdfstandards/pdf/PDF32000_2008.pdf
The spans, header identifiers and scope express cell relationships, independent
of replacing a cell's visible text while retaining its structure and geometry.
Verification: failing unit reproduction first, defensive cases, focused native
context regressions, current CLI on immutable originals, independent full tag
graph and fixed-region pixel/text checks, fresh scoped review. Native code change
requires a new candidate; accepted0.29 binary remains the pre-fix baseline.
