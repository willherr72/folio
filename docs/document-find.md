# Find in a document

Open Find with **Ctrl+F** or the Find button. Search covers readable page text and
added text. **Match case** and **Whole words** belong to the current tab, along
with its query and selected result. Changing either option starts at the first
match. Enter/F3 moves forward; Shift+Enter/Shift+F3 moves backward and wraps.
Escape closes Find. Repeated Ctrl+F selects the current query; Ctrl+Tab still
switches document tabs while the query has focus.

Case-insensitive document Find uses shared Unicode17 case-folding data in the
desktop and frontend paths. Greek sigma spellings agree, and `STRASSE` finds
`Straße`. A query cannot match only half of a character expansion created by
folding: searching for `s` does not select half of `ß`.

Whole words uses Unicode letters, marks, numbers and connector punctuation, plus
join controls. Internal straight/curly apostrophes stay within a word; hyphens
separate words. Contiguous CJK letters form one token. This is a deterministic
word-character filter; dictionary segmentation, accent removal and canonical
normalization are separate work. Whitespace in document queries is collapsed,
so phrases can span source line breaks. Paragraph Find/Replace retains its
existing draft-specific matching and replacement behavior.

Protected documents use the native find-only service. Extracted protected text
stays native; results contain locations and status. Long queries report an error
without dispatching work. Native per-page result limits remain visible so the
query can be refined. Searching leaves PDF contents and document undo history
unchanged.

The current source component passed authored model, native and compiled Windows
checks. The completed0.52 portable predates these controls; packaging and launcher
selection are separate. Physical keyboard/IME, general language/producer/reader
coverage, printing, accessibility and broad performance remain open acceptance.
