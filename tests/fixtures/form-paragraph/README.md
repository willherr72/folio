# Form paragraph producer fixtures

Generated with scripts/generate-form-paragraph-fixtures.py using the versions and
hashes in manifest.json. Original text/geometry authored locally; no private or
third-party document content. DejaVu Serif font license is recorded in the manifest.

standard.pdf uses ReportLab Platypus Paragraph with color spans. embedded.pdf uses
embedded DejaVu Serif; mixed.pdf combines Helvetica and embedded Serif, color and
an accented word. Each paragraph is in a shared Leaf inside a Wrapper, placed twice
on page1; page2 is independent. imported.pdf uses PyMuPDF show_pdf_page to wrap the
mixed input in imported-page Form XObjects. The generator refuses to overwrite a
nonempty output folder; pass another output folder for intentional regeneration.

Native tests live in src-tauri/tests/form_paragraph.rs. Exported artifact checks:
python scripts/verify-form-paragraph-corpus.py <artifact-root>. This is a small
producer corpus, not a general PDF-compatibility or performance certification.
