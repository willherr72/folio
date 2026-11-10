# Stable existing-text inspection for automation

The next automation stage lets a script or assistant select existing PDF text
without guessing coordinates or reusing a desktop process's temporary document
IDs. The user delegates routine design choices and wants Folio's text-editing
advantage available through CLI/MCP. This is the inspection prerequisite;
validated replacement previews and exclusive new-copy saving follow separately.

## Approach

Use the desktop `list_occurrence_text_runs` API, which understands complete-source
publication and its restrictions. Reusing ephemeral native IDs would fail across
the MCP worker restarts; matching text alone is ambiguous. Instead, derive opaque
`text-v1-<sha256>` selection IDs from a domain separator, loaded source-byte hash,
1-based page and the complete serialized native run. Identical loaded bytes and
inspection data produce identical IDs, regardless of filename or worker lifetime.
Changed bytes, page, index, text, geometry or support data invalidate IDs. These
are stale-selection checks, not secret capabilities or authorization to edit.
Future editing must regenerate the ID and rerun native replacement proofs.

## Interface and constraints

- `folio-cli inspect-text INPUT.pdf --page N [--offset N] [--engine PATH]
  [--password-stdin]`; page required, 1..10000, offset default 0, 0..100000.
- MCP `folio_inspect_text {documentId,page,offset?}` uses launch snapshots and the
  same disposable CLI worker. No model-supplied paths. Read-only, no persistent
  output folder or source registry. Existing worker cancellation/budgets apply.
- Return page, physical page width/height, coordinateSpace `displayed-page-points`,
  sourceFingerprint (SHA256 of loaded engine bytes), offset, totalRuns, nextOffset
  (null when finished), optional page reason and up to 100 native runs. Each run
  retains all native fields and adds selectionId. Bounds use top-left display
  coordinates in physical PDF points, including page rotation/crop/UserUnit.
- Native sourceText, when present, is exact operand text; text remains extracted
  text. Do not trim, normalize, truncate or merge it. Supported is preliminary
  eligibility, not a promise that a proposed replacement fits.
- Limit each serialized run to 16 KiB and the result to 2 MiB; fail explicitly
  instead of returning incomplete text. A fixed 100-run batch bounds transport,
  not native inspection cost. Offset beyond total fails; offset equal total gives
  an empty final batch. Blank/unsupported pages return native reasons.
- Enforce copy_content and edit_content before returning text. Native edit
  checks remain intact. CLI password entry follows the existing stdin contract;
  MCP password entry is not added. Owner rights follow engine policy.
- Do not add dependencies, alter native fidelity policy, flatten documents,
  expose native document UUIDs, or claim OCR/form-XObject discovery. This initial
  command lists top-level text only; nested-form inspection remains future work.

## Acceptance

Actual CLI subprocesses must show stable IDs across restart/rename, different IDs
for duplicate runs/pages/changed sources, exact parity with native inspection,
physical geometry, lossless whitespace, pagination, unsupported/empty pages,
permission denial and strict arguments. MCP must exercise the actual CLI worker,
closed schemas, read-only annotation, stable IDs and unchanged launch snapshot
after the original changes. No export folders persist. Bounded serialization
gets a focused regression. Package with the exact existing renderer and verify
modern/legacy official SDK inspection from outside the checkout.

Standing constraints: preserve all dirty work, source PDFs, prior artifacts/apps;
one Cargo job at a time with shared target; no commits/push/releases/cleanup or
commercial-parity claim. Execute inline, one fresh scoped final review.
