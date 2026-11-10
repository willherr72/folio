# Password protection for an edited PDF copy

The next quality gap is creating, replacing and removing PDF password protection.
The preceding on-page form checkpoint is verified; the broad commercial-quality
goal remains active. User authorization covers autonomous design and implementation.

## User workflow and choices

Add **Security settings…** to Save options. A dialog exports the current edited
document with either new password protection or no password protection. The current
tab, its protection, dirty state, undo and recovery stay unchanged. Ordinary Save
continues preserving original protection. The output remains an editable PDF copy.

Password mode has an optional opening password and confirmation, a required owner
(permissions) password and confirmation, and controls for printing (none/low/high),
copying, content editing, annotations, form filling and page assembly. Default all
actions allowed. Annotation permission also permits filling per PDF semantics;
the UI reflects this dependency. Opening and owner passwords must differ after
normalization. At least the owner password is nonempty. Explain that restrictions
depend on reader enforcement and that forgetting an opening password prevents access.
No invented password-strength score or promise of DRM.

Use AES-256, standard handler revision 6, encrypt metadata, fresh OS-random 32-byte
file key per export, PDF 1.7 Adobe extension level 8 (or preserve a newer version).
Use the existing pinned lopdf implementation; add direct dependencies on already
locked getrandom 0.3.4 and stringprep 0.1.5. Validate raw inputs <=1024 UTF-8 bytes,
SASLprep validity, normalized <=127 bytes, nonempty owner and distinct normalized
passwords before composition. Never silently truncate a password. Unicode is supported
under the same normalization used by the existing revision-6 reader.

## Authorization and fidelity

Every contributing protected source must have native owner authorization to change
or remove security, even if its reader permissions allow all editing. UI guards
explain Unlock editing; native policy remains authoritative. Mixed protected inputs
may be exported under an explicit new policy only when every source is owner-unlocked.
Ordinary saving retains the existing mixed-protection refusal. Prepared print sources
remain ineligible. Unknown sources, source-file destinations and malformed settings
fail before publication. Source files are never overwritten.

Use the existing bounded composition, forms/comments/links/bookmarks preservation,
physical units, editable overlays, cancellation and atomic publication paths. Add
the explicit security choice only to controlled save, not ExportRequest or persisted
workspace data. Authorization precedes copying. Encryption/removal occurs after
composition and before publication. Cancel before or after encryption preserves the
destination; individual crypto operations remain cooperative cancellation boundaries.
No plaintext intermediate file is written. Secret-bearing options have redacted Debug,
are not persisted, are never included in progress/errors, and are dropped after use.
Dialog password state clears after export/cancel; no guaranteed memory-erasure claim.

## Acceptance

Native tests: new revision-6 copy opens with distinct reader/owner credentials and
correct effective permissions, wrong/empty credentials fail when opening password
is set; permission-only mode; Unicode normalization/length collisions; randomized
encryption; new credentials replace old ones; removal requires owner; reader forgery
and mixed-owner/reader inputs denied; mixed owner inputs allowed explicitly; original
ordinary save policy unchanged; cancellation and destination/source integrity.

UI tests: confirmations, permissions dependencies, owner guard, pending/cancel/retry,
no secret persistence, unchanged dirty/undo state, focus and native IPC option routing.
Full frontend/build and relevant native protection/save/form/bookmark gates. One
scoped final review, coordinator corrections with regressions, no repeat review.
Windows acceptance on a fresh isolated build: protect an edited form, cancel/retry,
open as reader/owner, change/remove protection, save/reopen, narrow-window keyboard.
Independent MuPDF/pypdf verify encryption/permissions, credentials, values, links,
bookmarks and pixels; original hashes preserved. Publish only a local checkpoint.

This is password security management, not digital signing, certificates, XFA,
redaction, or proof of commercial parity. Those remain in the broader checklist.

References inspected: Adobe password security documentation and lopdf 0.45.0
encryption API/source; local vendored implementation is authoritative for this build.
