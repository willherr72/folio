# Folio's lopdf compatibility patch

Base: the exact crates.io lopdf0.45.0 archive, SHA256
`bfffda0fe1ab0157e1a13c14bebd3f28671f2fccb7922f0722ec53926e6922d3`.
The upstream MIT license remains in LICENSE. This is a local modification, not an
upstream release or endorsement. Version/dependencies are unchanged.

Folio's protected-document regression fixtures exposed these defects:

1. Revision2â€“4 owner authentication succeeded but file-key derivation used the owner
   password as a user password. Reuse the existing Algorithm7 recovered user value
   when deriving the file key; authentication still verifies that recovered value.
2. Stream bodies were encrypted/decrypted while strings inside their dictionaries
   were skipped. Apply the existing recursive primitive to dictionary values too,
   including metadata dictionaries whose stream bodies are deliberately clear.
3. The encrypted loader ignored object/decryption/object-stream errors. Propagate
   them instead of publishing incomplete or still-encrypted object values. Apply
   the same password sanitization already used by authentication during key setup.
4. AES-256 owner-key derivation skipped the encrypted permissions check; the user
   branch compared only three of its four permission bytes. Validate the complete
   block for both credentials, including all four permission bytes.
5. Accept a direct encryption dictionary as emitted by MuPDF's encrypted
   cross-reference streams, in addition to an indirect dictionary reference.

Also adds opt-in `LoadOptions::skip_decryption`, defaultfalse, so Folio can validate
the original encryption dictionary before implicit empty-password loading removes
it. That option returns encryption metadata only for protected inputs, never a
renderable decrypted document. Unencrypted loading remains unchanged.

Also adds `EncryptionState::from_authenticated_file_key` for resuming securely
retained state without retaining passwords. This constructor is NOT a password
authentication API or an owner-authorization decision: callers must supply an already
authenticated key. It validates key length/metadata and AES256 Perms using existing
library code. Folio calls it only after current-user DPAPI authentication, source-byte
hash binding and strict private context validation. Revisions2–4 rely on that trusted
state boundary. Ordinary decode still authenticates/derives the password key first;
it then shares the same state reconstruction. No cryptographic primitive changes.

Exact edits and upstream/patched file hashes are in FOLIO_PATCH.patch and
FOLIO_PROVENANCE.json. Run `python -B scripts/verify-lopdf-vendor.py` from the Folio
root; pass `--archive PATH` if the pinned crate is not in Cargo's cache. After
intentional changes, `--refresh` regenerates provenance and the exact patch.
The helper does not modify library source. The upstream `assets/example.pdf` test
fixture, omitted from the published archive, is retained from the recorded upstream
commit with its own URL/hash. A gitignore exception keeps it in the repository.
Local attributes disable newline conversion so provenance hashes remain stable
across Windows checkouts, including untouched upstream text files.

Run upstream encryption unit tests with:
`cargo test --manifest-path src-tauri/vendor/lopdf-0.45.0/Cargo.toml --lib encryption:: --no-default-features`.
The upstream crate's unpublished dev lockfile is generated locally for this command;
Folio's locked dependency resolution remains in src-tauri/Cargo.lock.
Before a future upgrade, rerun
Folio's pdf_protection integration tests, upstream encryption tests and independent
pypdf/MuPDF checks; remove a patch only after the corresponding regression passes.
Do not silently replace the patched source with the registry crate.
