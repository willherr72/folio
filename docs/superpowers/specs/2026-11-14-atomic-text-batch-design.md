# Atomic text-edit automation

The broader Folio goal remains an editor of commercial usefulness. A required
step toward document-wide replacement is a transaction that can change several
inspected text objects without leaving earlier changes committed when a later
font, collision or preservation check fails. This component delivers explicit
batch edits through CLI/MCP; automatic document Find/Replace planning remains
subsequent work, rather than being represented as already available.

The user has authorized autonomous inline execution and opted out of routine
design questions. Retain Rust/Tauri, the existing native editing engine and local
automation. No cloud service, new dependency, version/package/launcher change,
clipboard operation, private input, Git mutation or cleanup.

Two approaches were considered. A new whole-document stream-rewriting algorithm
would need to re-prove font, graphics, marked-content and form ownership. Instead,
run the established complete-source edits inside one worker transaction, owning
and closing all intermediate sources. This costs repeated validation/serialization
and makes no bulk-speed claim; it retains the stronger existing admission checks.

Native `TextBatchEdit` identifies a zero-based source page, an object path (one
index for top-level text, 2..13 for nested text), exact expected text, replacement
and optional explicit font ID. Accept 1..64 edits; at most 32 distinct pages;
expected text at most 16 KiB, replacement 1..4096 UTF-8 bytes, aggregate text and
font strings at most 512 KiB. Reject duplicate or prefix-overlapping paths on the
same page. Original paths must resolve uniquely before mutation. Apply each page's
paths in descending lexical order so removing a higher object cannot retarget a
lower one. Before every step, compare the still-unedited target with its original
native record; uncertain rebinding refuses instead of choosing nearby text.

Every step uses the current native complete-source replacement, including its
permission, font, layout, glyph, raster and PDF graph checks. Preserve page order,
source credentials/permissions, forms, links, annotations and shared-form
ownership. Keep the original source unchanged. On error or an abandoned reply,
release all unpublished candidates. Return one complete-source candidate only on
success, including a validated no-op; no-op automation publication must retain
the captured original bytes exactly. Native bounds remain in their existing units;
public document dimensions retain the normal physical-point conversion.

CLI `preview-text-batch INPUT --request EDITS.json --out-dir DIR [--font FONT]`
accepts strict `{version:1,edits:[existing TextEdit...]}` JSON at most 512 KiB.
Selection IDs refer to the same original inspected snapshot; do not resolve later
edits against an already changed fingerprint. MCP `folio_preview_text_batch` uses
the same schema and launch-authorized font/credential rules. Its result is a
normal session-owned preview capability, usable by existing Save/Apply/Discard.

Preview manifest version 6 adds bounded batch metadata to the existing envelope.
Retain every selected original ID/text/replacement/font and before/after images
for each distinct affected page, using generated filenames rather than client
paths. The first page retains the legacy before/after fields. All other page
proofs participate in validation, adoption, tamper detection and bundle budgets.
Manifest/request limits are 512 KiB, individual PNGs 32 MiB, and the complete
bundle/session budget remains 512 MiB. Prior versions 1..5 remain readable and
must reject batch metadata. JSON failures must not echo document contents.

Acceptance needs real multi-run/same-page, multi-page and nested-form edits,
late collision/font failure with unchanged originals and released intermediates,
no-op byte identity, duplicate/stale/oversize requests, protected credentials and
permissions, exact saved candidates, generic MCP chaining/Discard/Apply, complete
image tamper checks and independently checked saved PDFs. Use authored public
fixtures only. Native and frontend evidence retain their real profile meanings.
One final fresh feature review follows the whole implementation, with one
Critical/Important RED/GREEN fix pass and no re-review.
