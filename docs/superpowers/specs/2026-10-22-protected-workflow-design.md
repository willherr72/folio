# Protected document workflow

## Intent and authorization

Continue the autonomous quality goal with an end-to-end usable protected-PDF flow:
open with a password, edit within permissions, save with original protection, recover
without passwords or readable document data on disk, and print within granted rights.
This is architectural work under brainstorming/writing-plans/executing-plans. The
user explicitly authorized autonomous decisions without routine approval gates;
continue inline in the existing isolated worktree, with no commits or publication.
Prerequisites: protection codec8a and account-protected recovery8b are verified.
The full lifecycle is the target; individual tasks are progress, not parity claims.

## Chosen approach

Compose PDF output in memory before applying protection and atomically publishing.
Encrypted temporary staging would require decrypt/re-encrypt passes between current
path-based processors and risks readable intermediates. Rejecting all protected edits
would preserve safety but would not provide the requested workflow. A shared in-memory
PDF document lets forms, editable annotations and bookmarks finish before one final
bounded serialization. Print and composed preview consume in-memory bytes directly.

## Composition boundary

Keep page geometry, annotation rendering, form/action transplantation, bookmarks,
source overwrite guards and atomic destination replacement. Move AcroForm preservation,
portable finalization and outline writing onto the same lopdf Document. Preserve public
path-based bookmark writer compatibility through a bounded wrapper. PDFium and lopdf
output writers cap serialized bytes at512MiB, with sticky failure and no partial output
publication. This is a byte-output bound, not a whole-process memory guarantee.
Preview and print preparation create no PDF files. Print-only annotation flags and
prepared-source cleanup still apply. A synthetic path may identify an in-memory print
source but is never created or loaded. Windows printer spooling is the explicitly
requested print destination, distinct from an application plaintext staging file.

## Runtime protection and recovery

Every opened protected source retains opaque authenticated context and rights alongside
its plaintext bytes. Missing/incorrect passwords are typed challenges; malformed or
unsupported protection remains a refusal. Empty-user-password documents receive the
same rights/encryption preservation as password-required documents. Editing replacements
inherit their origin context, rights, document identity and page provenance. Plain
sources remain unchanged in behavior.

Retain file-key/algorithm context, not plaintext passwords. Private recovery stores any
serialized authenticated context only inside its DPAPI-protected native manifest/source
payload; frontend DTOs and diagnostics never contain keys. Validate bounded context
schema/algorithm/identifiers when restoring. A missing/invalid context for a protected
source must never downgrade it to unprotected. Owner authentication is retained only
inside authenticated recovery state. Account/profile loss retains existing recovery
error/discard behavior.

## Native policy and UI

Native commands enforce copy, content editing, annotation editing, form filling,
assembly and printing independently. Frontend disabled controls explain the same policy
but are not its enforcement. Read-only viewing/search must not become an unrestricted
copy endpoint. Track original overlays/page order/rotations and source provenance so
export cannot bypass restrictions by sending a hand-edited page plan. Annotation-only
permissions allow annotation changes, not flattening them into edited page content.
Form filling inherits protection and does not grant content editing.

Derived copies/extractions retain the original encryption, both passwords, identifier
requirements and permissions. Reject combining incompatible protected contexts with a
clear explanation until the user has an explicit protection-conversion workflow; never
choose credentials arbitrarily or silently drop protection. Unprotected additions to a
protected workspace use the protected output context only when allowed by assembly.
Print preparation checks printing permission and uses limited raster quality when only
low-quality printing is allowed. Owner unlock grants owner capabilities without erasing
original output permissions. Unsupported signed/public-key/XFA workflows stay explicit.

Password UI: masked input, wrong-password retry, cancel with no source leak, no retained
password in workspace/local storage/logs. Native opaque pending-open identity binds the
challenge to the chosen file; do not expose arbitrary file-reading through a retry DTO.
Expose a nonsecret permissions summary and owner-unlock action. Guard stale async opens,
closing tabs, undo/redo, recovery and source release. Ordinary unprotected flows keep
existing affordances and tests.

## Acceptance and review

Meaningful RED/GREEN tests cover each native boundary and UI challenge lifecycle.
Shared composition tests cover forms+overlays+bookmarks together, exact geometry/pixels,
preview/print rendering and no plaintext staging. Boundary tests use small configurable
writer limits rather than allocate512MiB. Relevant suites run as tasks finish; full
native/frontend/production/embedded desktop checks at the lifecycle integration gate.
Synthetic protected fixtures span Standard revisions2–6, owner/user/empty passwords,
restricted permissions, source replacements, page extraction and incompatible mixes.
Independent pypdf/MuPDF confirm passwords, permissions, content and unchanged surrounding
pixels. Actual isolated Windows password retry/cancel, edit/save/reopen, forced-exit
recovery and virtual-print/cancel checks complete the flow. Originals remain hash-stable.
One fresh review of the completed lifecycle implementation; reproduce/fix material
findings once and record deferred limitations. No claim of Adobe/Foxit parity follows
from this milestone alone.
