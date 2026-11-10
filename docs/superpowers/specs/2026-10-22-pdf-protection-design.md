# Protected PDF lifecycle

## Intent

Support everyday protected PDFs as part of the ongoing Adobe/Foxit usefulness goal:
open with a password, honor the authenticated user's permissions, edit supported
content, and save/recover without silently dropping document protection. This is
not satisfied by a password dialog followed by unrestricted plaintext export.

Existing source parsers, native text/form replacements and export all operate on
unencrypted source bytes. Recovery separately stores source PDFs, fonts and workspace
JSON. Implement and verify the encryption-preservation prerequisite before wiring
these paths together; no claim of user-facing password support until integration.

## Cycle 8a: native protection codec

- Add a focused `protection` module using the pinned lopdf implementation. No custom
  cryptography or dependency upgrades. Support Standard handler RC4 revisions2/3,
  AES-128 revision4 and AES-256 revisions5/6; reject unsupported handlers/filters.
- Decode to in-memory plaintext plus an opaque cloneable protection context. Retain
  original credentials indirectly through encryption state, first trailer ID,
  algorithm parameters and permissions; never expose key/password bytes via DTOs,
  Debug output or errors. Unprotected inputs return their original bytes unchanged.
- Authenticate owner and user separately, including empty user passwords; an owner
  password grants owner capabilities while a user password retains document limits.
  Report typed missing/incorrect-password errors, distinct from unsupported/invalid
  documents. Password strings are transient inputs, not stored in the context.
- Reprotect a transformed plaintext PDF with its original context so both original
  passwords and permissions still work. Preserve the first document ID required by
  RC4/AES-128 key derivation. Preserve relevant encryption version declarations.
- Keep permission interpretation separate from capability enforcement; integration
  must enforce these rights in native commands as well as the UI. Printing quality,
  copying, annotations, filling, assembly and general content editing are distinct.
- Bound input/output at512MiB, objects100,000, per-stream decode64MiB and password
  input1,024 UTF-8 bytes. Reject malformed/unsupported protection rather than quietly
  returning an unrestricted context. Do not write plaintext or ciphertext to disk
  inside the codec; callers own publication and recovery lifetimes.
- Fixtures: synthetic independent pypdf inputs spanning the supported algorithms,
  owner/user/empty passwords and restrictive permissions. Tests cover transformed
  documents (renumbered objects and changed content), Unicode passwords, metadata
  choices, compressed objects, malformed inputs and unaffected plain documents.
  Independently check output authentication, text and rendered pixels with MuPDF.
- Clear metadata retains encrypted dictionary strings and an explicit Identity
  stream filter in protected output; remove the transport filter in plaintext.
  This first codec accepts unfiltered metadata and a single Identity Crypt filter.
  Other clear-metadata filter chains and general per-stream Crypt overrides fail
  explicitly. See ISO32000-1 section7.6.5:
  https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf

Implementation required a maintained patch of pinned lopdf0.45.0: owner-key
derivation for legacy revisions, stream-dictionary strings, complete AES-256
permission validation, direct encryption dictionaries and encrypted-load failures.
The original algorithms remain in use. Exact source/patch provenance and upstream
license live under src-tauri/vendor/lopdf-0.45.0; scripts/verify-lopdf-vendor.py
checks the pinned archive and local edits. Library patching is an ongoing maintenance
obligation, not a replacement for independent reader validation.

## Following integration cycles (required before password support is complete)

1. Runtime retains protection context across immutable source replacements; transport
   a typed password challenge, cancellation/retry and native capability checks.
2. Permission-aware editing and exports: preserve encryption for derived copies;
   combining different protection contexts needs an explicit policy, not arbitrary
   inheritance. Protect the final output before atomic publication, including temporary
   outputs; rendering/printing must not leak plaintext files or bypass print limits.
3. Recovery must protect sources, overlay/workspace text, fonts and authentication
   material at rest. Evaluate Windows account-protected storage and restore behavior
   before implementation; never retain plaintext passwords in recovery JSON.
4. Windows password-entry UX, wrong-password/cancel/retry, owner unlock, rights display,
   save/reopen and forced-exit recovery. Existing PDF fidelity and original hashes
   remain acceptance gates. Signed and public-key encrypted PDF workflows are separate.

## Nonclaims

Cycle8a alone does not add password controls, secure recovery, access enforcement,
new-password authoring or protected export. Those remain explicit implementation work.
No application parity or security certification is implied by passing codec fixtures.
