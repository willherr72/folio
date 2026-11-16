# Source font guidance

The original private quiz still requires explicit font substitution when an embedded subset lacks a new character. The current chooser hides the source font context and presents an alphabetical catalog. Help the user find a sensible starting face without claiming font equivalence or changing the PDF automatically.

Use an optional `sourceFontName` on FontPicker. Show the source label as text and explain that name matches are starting points requiring PDF preview. Move supported exact normalized name matches before the rest of the filtered catalog, preserving stable order in each group. Mark those rows with a separate visible/accessibly described “Source name match” cue. Keep the button's accessible name equal to the installed font name.

Normalize both names with NFC, strip exactly one leading six-uppercase-letter PDF subset tag followed by `+`, lowercase without locale-specific casing, and remove spaces, comma, underscore and hyphen separators. Do not remove style tokens, infer aliases, perform fuzzy matching, or conflate regular/bold/italic. Whitespace-only or missing names never match. Unsupported entries stay disabled and are not promoted. Search retains its existing behavior and the full catalog remains available.

Pass the PDF source name from single-run, uniform paragraph and styled paragraph editors. For a styled selection, offer a source name only when all selected source styles have the same exact display label; mixed source fonts have no misleading singular hint. An explicit substitute already selected does not replace the original source label. New text/other font chooser clients keep their existing behavior.

Approaches: an initial search would hide alternatives and overfit PostScript naming; automatic loading would consume resources and imply a selection. Stable name-match ordering with context is the chosen small frontend-only approach. Name matching never bypasses native font loading, character coverage, preview, embedding permissions, collision checks or explicit Apply.

Preserve original PDFs, selected 0.50, reviewed 0.51 archive/executables and all failures. Keep private quiz input/output under ignored private QA; public regressions use fabricated names/text. No clipboard mutation, new native change, package promotion, commit, deletion, merge or push. The system paste/physical keyboard/IME gates stay open. This component does not claim commercial parity.
