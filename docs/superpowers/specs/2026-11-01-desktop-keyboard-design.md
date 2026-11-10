# Native desktop keyboard editing acceptance

Close the integration gap left by composition-shortcuts component acceptance.
Build the current frontend into an isolated debug custom-protocol Folio executable,
then exercise actual native text editing, metadata editing and save/reopen at
840x640. Use the existing public two-page dependent-text fixture unchanged: New
content, a following Suffix/Neighbor and an untouched second page make collateral
change visible. Use owned copies and exclusive output paths only.

Check both synthetic composing Escape representations in the existing-text and
document-properties dialogs; verify normal Escape cancels drafts, Tab can reach
actions, keyboard Enter applies, document Undo/Redo and saved re-edit work. Search
must ignore composing Enter/Escape and retain ordinary Enter/Escape behavior.
Apply a Unicode document title and verify it after actual reopen and independent
metadata extraction. Do not require source-font glyphs absent from this fixture.

Independent MuPDF/pypdf checks compare complete extracted words, font/size/color/
baseline, unchanged glyphs/pixels outside a fixed original-space text edit region,
full second page, metadata and preserved inputs. Include intentional corruptions
that must fail the relevant audit. Reuse the established fixed-mask text audit.

Approach: native candidate and owned WebView2 test session rather than another
component-only harness, because the missing evidence is full application behavior.
Use CDP keyboard events plus synthetic composition flags; this still does not
certify installed OS IME candidate windows. No new portable version, full native
regression, broad accessibility, physical printing or parity claim. Existing
native evidence is qualified by unchanged engine/dependency hashes. The user has
authorized autonomous verification and routine fixes; no new permission needed.
