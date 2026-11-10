# Mixed paragraph suggestions

The quiz's highlighted line consists of three adjacent text pieces. Current
fresh styled suggestions return just the clicked piece, forcing manual selection
before a safe edit. The existing uniform suggestion algorithm already groups
contiguous reading-order pieces and aligned nearby rows. Extend that algorithm
with an explicit mixed mode, leaving uniform behavior unchanged.

Saved styled memory retains first priority and complete verification. Fresh
suggestions use current native text geometry, never metadata membership alone.
Group horizontal adjacent pieces on the same baseline across font/size/color
changes. For multiline groups, use each row's largest font for leading admission
and require a body font size shared by every row in the block. This admits body
text with larger inline emphasis without joining a separate differently sized
heading. Keep existing column gaps, list markers, indentation/alignment, shifted
baseline, reading order and 64-piece boundaries. Do not infer semantics from
private words or add OCR/complex-script support.

The proposed group must pass `inspect_styled_paragraph`, including source/font,
ownership, stream, line/selection and copy-raster checks. If geometric grouping
or group inspection fails, preserve current clicked-piece inspection and manual
selection. Stale anchor identity remains an error. No publication gate changes;
all preview/apply, glyph, background and surrounding-content checks remain.

Area fitting after text insertion remains a distinct subsequent usability task.
This change makes a complete candidate easier to reach; it does not silently
expand a changed draft or substitute a font. The paragraph editor shows the
selected text/count and keeps Adjust selection available before editing.

Acceptance: native authored cases for same-line mixed highlights, wrapped mixed
sizes, crop/rotation, column/list/heading/superscript boundaries, stale anchors,
saved and forged memory, unchanged source bytes; existing uniform suggestions
regressions. Numeric-only quiz diagnostics should verify consistent complete
membership from every anchor on the highlighted line, including supported wrapped
continuations. The initial three-piece row was a manual subset; native geometry
identifies seven pieces over three centered rows (approximately 14.64pt baseline
spacing), and unchanged/insertion previews succeed in the suggested area.
Frontend tests verify the returned complete selection
continues into preview and adjustment. Review and recorded limits required.
