# Uniform paragraph editing

The first paragraph editor combines selected existing PDF text pieces, edits their
text, and rewraps it inside a fixed area. The user approved uniform formatting:
one font resource, effective size, and color. Mixed formatting remains unchanged.

The dialog checks the selection before editing, shows the combined logical text,
allows width and height changes, and renders a native PDF preview before Apply.
Overflow keeps the draft. Apply commits the exact preview as one undo step.
Cancel, stale requests, and failed previews release temporary sources.

The native implementation retains the original font and removes the original
selected characters from the content stream. Numeric text advances preserve the
positions of subsequent unselected text. New lines are appended with explicit
text state inside a saved graphics state. Reopened text, style, geometry, and
pixels outside the edit area are checked before a candidate is published.

Initial support is bounded horizontal Latin text on unrotated source pages,
with verified embedded fonts or standard PDF fonts. Selected pieces must follow
reading order and share formatting. Source line breaks become spaces; explicit
newlines in the replacement remain hard breaks. No font shrinking, paragraph
pushing, page flow, font substitution, or mixed formatting conversion occurs.

Tests use generated PDFs and redistributable fonts. The supplied quiz is a local
acceptance case only; the original is never modified or committed.
