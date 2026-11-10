# Folio's native call serialization correction

Base: the published pdfium-render 0.9.4 archive, SHA-256
`8948a803616a9e936b15a6637af2cd48c5fb8ae0fcdeb3c32eac3a540e255a19`.
The upstream dual-license notice and published source files are retained.
The archive contains a short LICENSE.md linking the MIT/Apache alternatives; Folio
also includes the complete Apache-2.0 text from the Apache Software Foundation for
distribution under that option, with its source and hash recorded in provenance.
This is a local modification, not an upstream release or endorsement. Dependency
versions, public API and enabled features are unchanged.

PDFium requires serialized calls. The upstream thread-safe wrapper guards most
delegated calls, but omits the same guard on 192 other methods/configuration
variants. These include matrix rendering, bitmap creation, form initialization,
and text/font operations. Independent documents can therefore enter PDFium
concurrently despite the enabled thread_safe feature. Folio's parallel choice
tests reproduced heap corruption, access violations and inconsistent rendering;
the same 43 tests pass serially. Production Folio also retains its single native
worker; this correction does not introduce concurrent document editing.

The patch adds the existing process-global mutex guard for the full duration of
every omitted delegated call. Each changed method makes one call to the underlying
binding. Existing guarded methods remain unchanged. It corrects the stale opening
comment to describe the per-call locking already used by the implementation.
Callbacks must continue not to re-enter PDFium through this binding, as documented
by upstream. Raw buffer lifetimes and access to the same native handle remain the
caller's responsibility; serialization does not grant unrestricted shared handles.

The local .gitignore preserves all published files, including Cargo.lock and docs,
so the maintained inventory can be verified exactly. Verify with
`python scripts/verify-pdfium-render-vendor.py`; after an intentional source change,
`--refresh` records the exact patch and source checksums without editing library
source. Cargo.lock selects this local package. Build fingerprints include its Rust
sources and manifests, and portable notices retain this file, the patch, provenance,
the upstream license notice, and the complete Apache-2.0 text.

Before replacing this fork, run the unmodified parallel `engine::choice_` library
tests, the full Folio library suite (including shared-worker recovery tests), and
native integration/desktop acceptance against the proposed binding. The original
failure and diagnostic evidence are recorded in the form-border-precision milestone.
