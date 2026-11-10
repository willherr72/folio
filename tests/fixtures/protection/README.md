# Protected PDF fixtures

These PDFs contain only synthetic text and vector shapes created by `generate.py`.
They use built-in Helvetica; no private PDF, external font or personal data is copied.
All fixture passwords are public test data recorded in `manifest.json`.

Run `python -B tests/fixtures/protection/generate.py` to regenerate with installed
pypdf/PyMuPDF. Encryption salts and file identifiers are randomized: regeneration
changes bytes and refreshes the SHA256 manifest. The generator versions are recorded.
RC4 variants and AES revision5 are legacy compatibility cases, not recommended new
document protection defaults. Test output must retain original credentials/rights.
The `edit_only_*` fixtures distinguish content editing from page assembly: bit4 is
set and bit11 clear. Assembly follows bit4 only for revision2; later revisions use
bit11. Owner authentication retains its explicit permission override.

Set `FOLIO_PROTECTION_OUTPUT` to the absolute `artifacts/protection-codec-output`
directory before `cargo test --manifest-path src-tauri/Cargo.toml --test pdf_protection`.
Then run `python -B scripts/verify-protection-codec.py` with pypdf and PyMuPDF
installed. It checks fixture hashes, credentials, metadata, text and identical page
pixels. Generated files contain only public synthetic fixture content/passwords.
