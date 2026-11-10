# Stable paragraph inspection for automation

Folio's next automation workflow is paragraph reflow. Its first independently
useful step is inspecting the actual paragraph around a previously inspected text
run, including mixed formatting, inferred alignment/leading and verified highlight
cues. This milestone adds read-only CLI/MCP inspection; paragraph preview comes
next and must re-resolve the inspected source before accepting edits.

The coordinator chooses this scope under the user's continuing autonomous-work
authorization. Reuse the desktop complete-source styled paragraph suggestion API.
Flattening the paragraph to one style would discard source fidelity; guessing
paragraph membership in the automation layer would duplicate native admission.
Exposing raw object indices as caller authority would allow stale selections.

CLI: `inspect-paragraph INPUT --page N --selection-id text-v1-HASH`, with existing
engine/password options. MCP: `folio_inspect_paragraph` takes only documentId,
page and selectionId. Resolve the seed against the current immutable source,
requiring copy/edit rights, then call suggest_occurrence_styled_paragraph. A
run's one-line supported flag does not determine paragraph eligibility: the
native paragraph proof makes that decision. Existing single-run preview retains
its supported check.

Return page dimensions, displayed-page-points coordinate space, sourceFingerprint,
seedSelectionId, paragraphId and an exact `paragraph` StyledParagraphSelection.
The paragraph ID is domain-separated SHA256 over source hash, 1-based page and
bounded native selection JSON. It excludes process/session IDs and seed identity,
so two seeds selecting the same paragraph yield the same paragraph ID. Style IDs
are local to that paragraph ID, never global fonts. Bound serialization at 256 KiB
including final envelope; fail without truncation. Native selection size bounds
remain in effect. No export folders, previews or source mutations.

No new dependencies or native admission changes. This does not promise that a
replacement fits, support nested reusable-object paragraphs, or implement reflow
publication yet. Preserve prior preview/apply/save and all review artifacts.

Acceptance: real CLI and MCP subprocesses, stable selection across workers and
rename, source/page mismatch rejection, exact native mixed-style/geometry output,
protected source denial, strict arguments, output cap, no public files, package
renderer discovery and official SDK legacy/modern use on generated public Word
PDFs. Review paths and supported-vs-paragraph eligibility deliberately.
