# Windows account-protected recovery

## Purpose and scope

Cycle8b advances the protected-PDF lifecycle: all new Windows recovery payloads
(workspace text, source PDFs and font programs) must be protected at rest before
password-protected editing can be enabled. Recovery must work after process exit
without a plaintext temporary PDF. Preserve editable overlays, source identity,
font ownership, fallback generations, discard semantics and original-file guards.
The user authorized autonomous implementation; continue inline without approval or
commit/publish checkpoints. This does not replace the remaining PDF permissions,
password UI, context inheritance and encrypted export work in the lifecycle spec.

## Decision

Use current-user Windows DPAPI through the existing windows-sys dependency. A local
random encryption key stored beside recovery would not protect copied recovery
files; asking for a new recovery password would burden ordinary crash recovery.
DPAPI fits this Windows desktop product and its existing account-local data folder.
Do not use LOCAL_MACHINE scope or any prompt. No plaintext fallback on API failure
or unsupported platforms. Files can normally be recovered only under the same
Windows account; account/profile loss can make recovery unavailable. This does not
protect against processes running as that user or provide secure deletion of old
plaintext sectors. User-owned PDFs and portable packages are not migrated.

Microsoft reference: [CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
and [CryptUnprotectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptunprotectdata).
Use UI_FORBIDDEN, NULL prompt/reserved/description, current-user scope, LocalFree
for API buffers and clearing of temporary decrypted buffers. Include a SHA256 of
the plaintext inside the protected payload to detect corrupted output even when
the platform API does not return a specific tamper error.

## Storage and compatibility

A focused private_storage module seals/unseals versioned envelopes in memory.
Purpose (workspace/source/font), plaintext length and checksum are bound inside the
DPAPI payload; purpose-specific optional entropy prevents interchanging file kinds.
The external header holds format/purpose/lengths and a ciphertext checksum for bounded reads and fast cached
snapshot size checks. Unseal validates both external and internal metadata, exact
length, checksums and caller limit. The outer checksum detects accidental changes
to DPAPI header fields the platform may ignore; it is not authentication. Payload
cap512MiB; envelope overhead cap64KiB.
Existing workspace32MiB/font16MiB limits remain. Diagnostics contain no payload data.

Recovery keeps its existing owned filenames and generation numbering. Their contents
become binary envelopes, including files with historical .json/.pdf/.font suffixes.
Seal fully before atomic_write creates a temporary file. Only ciphertext reaches
temporary and final payload files. Lock/discard markers contain no document content.

On load/save, under the existing recovery mutex/OS lock, upgrade bounded readable
legacy owned payloads by atomically replacing them with sealed bytes. Already sealed
files are left unchanged. Interrupted migration leaves a mixture that the next pass
can finish. Unknown envelope versions must not be treated as legacy plaintext.
Migration respects path validation, never follows redirected files/directories,
and never recursively deletes or rewrites external files. Per-payload migration
failures cannot block unrelated valid generations or a new durable save. Required
failed payloads remain for retry if no generation restores; errors remain actionable.
After successful restore/save, existing garbage collection removes invalid generations
and unreferenced owned payloads. Cleanup remains best effort for filesystem failures;
inaccessible legacy leftovers cannot be guaranteed removed until access is restored.
Remove abandoned .folio-recovery-* regular temporary files under the lock in all
three guarded directories, including on discard and interrupted-discard restart.
Never follow or recursively remove matching directories. Discard remains available.
Legacy valid manifests retain their schema; after migration all runtime payload
reads require sealed envelopes. Corrupt generations still fall back to older valid
generations. No DPAPI failure may cause a plaintext write.

## Engine restore

Change the internal recovery-open request to accept verified plaintext bytes plus
the snapshot path and original path as identities. Refactor file opening to share
the existing post-read initialization. The worker must never reopen the ciphertext
snapshot or write decrypted data to a temporary path. Keep snapshot/original paths
as export-overwrite guards, including hardlink aliases, and retain immutable bytes
for the lifetime of PDFium. Current and legacy annotation/font import rules stay.

## Acceptance

- DPAPI round trip including empty/Unicode/binary data, different ciphertext per
  seal, wrong purpose, altered headers/payload/truncation, size limits and foreign
  envelope rejection; independent Windows ProtectedData unseal of a synthetic probe.
- Recovery files contain no source PDF signature, chosen PDF/workspace strings or
  font bytes. Restart restore works without original files, and no plaintext PDF
  or workspace temporary file is created during save/restore.
- Legacy plaintext and interrupted mixed migration recover; unchanged encrypted
  sources are reused; corrupt latest generation falls back; discard/locking and
  original-path/hardlink safeguards still pass. Corrupt fonts roll back registry.
- Relevant native tests, production/desktop builds, one fresh review and a real
  Windows edit/forced-exit/restore/save/reopen workflow in an isolated data directory.

## Remaining lifecycle requirements

This milestone protects recovery payloads already produced by the editor. It does
not yet retain PDF protection contexts or grant protected opening/editing. Next:
serialize authenticated context only inside protected storage, inherit it across
source replacements, enforce native rights, preserve protection on save/extraction/
printing and supply password entry. Full lifecycle remains the acceptance target.
