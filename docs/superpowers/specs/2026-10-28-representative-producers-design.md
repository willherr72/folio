# Representative producer compatibility investigation

The accepted 0.24 package proves a bounded six-document corpus of simple producer
text. The next cycle should identify concrete everyday editing failures using
more realistic documents before choosing an engine change. This investigation
adds evidence; it does not redefine the broader commercial-quality goal.

Generate entirely authored, public local fixtures with installed Word,
ReportLab and PyMuPDF APIs. Include wrapped body paragraphs, mixed-style text,
table cells/rules, a link, header/footer and a separate text box/callout. Include
an untouched second page. Word exports both tagged and untagged; other producers
exercise standard and embedded fonts. Keep original exported PDFs immutable.
Do not normalize/rewrite source PDFs to make them pass Folio.

Use the accepted extracted 0.24 CLI to inventory every nonempty visible run and
nested run, then attempt meaningful original-font text and paragraph insertions
in body, table and callout targets. Record refusals as data. For successful edits,
save and re-edit actual output, independently compare complete expected text,
font identity/programs, links, unaffected pages and pixels outside measured edit
bounds. Report discovery, mutation and fidelity separately. A successful CLI exit
alone is not acceptance, and empty renderer objects are not visible-text failures.

Use only a newly created Word COM instance and fixture document. Capture existing
Word processes first; do not change or quit any preexisting application. Hidden
owned instance only, no macros, recent-file additions, external navigation or
updates to source/user documents. Preserve failed attempts and all prior artifacts.
No product/test/build-input edits or Cargo during this investigation. Select the
next implementation scope from reproduced user-facing failures after inventory.

Existing producer generator is the local API reference. Word text boxes follow
[Shapes.AddTextbox](https://learn.microsoft.com/en-us/office/vba/api/word.shapes.addtextbox)
and [TextFrame.TextRange](https://learn.microsoft.com/en-us/office/vba/api/word.textframe.textrange).
Export uses the existing owned-process PDF workflow. No proprietary fonts or
generated private documents are added to a distributable package.
