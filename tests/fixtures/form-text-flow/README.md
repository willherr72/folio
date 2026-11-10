# Reusable text flow fixtures

Generated locally with ReportLab 4.5.1 by
`python scripts/generate-form-text-flow-fixtures.py`. The generator uses invariant
metadata, uncompressed streams, original short sentences and standard Helvetica.
No private or third-party PDF content is included. File hashes and generator
version are recorded in `manifest.json`.

Three producer patterns (`drawString`, `PDFTextObject.textLine`, and explicit
multiple line matrices) share font setup across empty text objects and later
shows. Each two-page original has repeated forms; first/later-line references
change only the first placement. Reference content is authored independently of
Folio's rewrite path. Regeneration is an intentional fixture update.
