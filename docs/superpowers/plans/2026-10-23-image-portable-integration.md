# Editable image persistence and ownership

> Execute inline with superpowers:executing-plans. The user authorized autonomous
> development; no further design or implementation permission is needed. Use a fresh
> focused review after the integrated native tests and independent-reader checks.

**Goal:** Connect the verified image resources to existing-page overlays, editable
PDF save/reopen, resource ownership and private recovery before exposing a picker.

**Prerequisites:** `2026-10-23-image-assets.md` and
`2026-10-23-image-pdf-appearances.md` are complete. The full feature contract is
`../specs/2026-10-23-image-placement-design.md`. Portable 0.16 remains untouched.

## Decisions from current code inspection

- `persistence::same_object` currently assumes generated streams are uncompressed,
  ignores Filter/DecodeParms and compares decompressed actual bytes to expected
  raw content. Image resources include compressed PNG/ICC streams and original
  DCT JPEG streams. Do not pass those through the old assumption. Add a bounded
  image resource comparison that retains encoding semantics: compare encoded bytes,
  Filter and DecodeParms for generated encoded streams, ignoring only Length.
  A third-party recompression may conservatively remain native. Preserve the existing
  lossless comparison behavior for uncompressed text/font resources. Test JPEG
  ColorTransform tampering explicitly; an appearance-content hash is insufficient.
- Keep the original encoded asset in a versioned, bounded annotation attachment,
  separate from the drawing's XObject resources. Validate its identity from bytes,
  regenerate the expected appearance, then prove the entire resource graph before
  admitting an editable overlay. Deduplicate assets/prepared forms in an import cache.
  Never register an asset based on metadata alone.
- Retain the outer appearance's existing 64 MiB limit. Image stream comparison must
  be bounded by locally regenerated expected streams, not by an untrusted length.
  Account separately for encoded assets and cached prepared resources. A 32 Mpixel
  16-bit image can exceed 64 MiB; do not blindly raise all persistence limits.
- Images use a top-left anchor, positive unrotated width/height and quarter-turn
  rotation, consistent with text's anchor convention. EXIF orientation is already
  inside the prepared unit-square form. Derive all displayed/PDF geometry from
  PageGeometry; cover nonzero crop origins and intrinsic plus workspace rotation.
- Images force the portable flatten path for text/ink/images on the same page to
  retain their relative order. Highlight/comment annotations currently use another
  path; establish and test the resulting order instead of silently changing it.
- Image acquisitions have independent lease tokens, unlike the older font-ID API.
  An opened source needs its own native lease; frontend history needs a distinct
  acquisition before that source can close. Pending imports and cancelled opens
  must release only their tokens. Borrowed Arcs survive concurrent lease release.
- `open_bytes` currently stages fonts until every page/policy check succeeds.
  Stage images too, publish batches atomically, and roll back either resource family
  if the other fails. Printing/internal preview opens must not publish editable leases.
- Recovery remains current-user DPAPI. Add image payload purpose/directories and
  manifest fields with legacy defaults; validate identities, lengths, references and
  content before acquiring leases. A failed restore must not remove existing owners.

## Task 1: Original asset attachment and resource proof

Files: new focused image persistence helper/tests; `persistence.rs` only where the
existing comparison must distinguish encoded generated streams.

- [x] RED tests: original PNG/JPEG attachment roundtrip and full resource matching;
  tampered bytes/id/length, DCT DecodeParms, ICC, SMask/depth, matrices, resource
  additions, cycles and oversized data remain native without registry publication.
- [x] Implement bounded cache and proof. Preserve encoded originals and deterministic
  prepared forms; reject unsupported metadata using the completed preparation path.
- [x] Verify repeated references reuse preparation and bounded admissions fail cleanly.

## Task 2: Native overlay, geometry and save/reopen

Files: `types.rs`, `engine.rs`, `persistence.rs`, new native integration tests.

- [x] Add ImageOverlay (id, assetId, x/y, width/height, rotation) and versioned portable
  metadata. Validate finite geometry, quarter turns, identity and required asset.
- [x] Add image appearance using the prepared unit form; save original attachment
  only for editable output. Flattened output contains drawing resources only.
- [x] Integrate export validation/permissions and portable rendering order. Reopen
  only proven images, transform frames correctly and leave third-party edits native.
- [x] Test alpha, orientation, all page rotations/crop origins, out-of-bounds refusal,
  mixed text/ink/image ordering, source immutability, protected outputs and print.
- [x] Independently render/inspect editable and flattened PDFs, asserting unchanged
  native text and pixels outside the placed image and no duplicate drawing on reopen.

## Task 3: Engine and frontend resource ownership contract

Files: `engine.rs`, `main.rs`/command bindings, native image APIs, frontend types and
resource lifecycle modules. No image placement controls yet.

- [x] Expose validated import/info/acquire/release operations with unique tokens.
  Raw byte access was removed after protected-source review; see ledger ruling.
  Return useful image dimensions/profile errors before an edit can be inserted.
- [x] Give source documents their own image leases; stage all open acquisitions and
  roll back on later page/policy/capacity failure. Closing releases only source leases.
- [x] Render image-bearing pages/thumbnails through native composition (including drag
  drafts), or prove an equivalent preview. Content drawings sit below annotation markup
  on image pages; browser originals alone do not prove ICC/layering agreement. Cache
  prepared image resources where profiling shows repeated decode/compression is costly.
- [x] Track pending imports, tabs and past/present/future history acquisitions; an
  obsolete async response cannot release a newer operation's resources.
- [x] Test duplicate sources, two owners, close/undo/redo, cancelled dialogs, discarded
  opens, pending unmount and capacity failures without invalidating active work.

## Task 4: Private recovery and integrated verification

Files: `recovery.rs`, `private_data.rs`, recovery fixtures/tests and lifecycle callers.

- [x] Persist encoded image payloads under DPAPI, deduplicated by content identity.
  Use native-generated relative payload names, checksums and bounded lengths.
- [x] Validate complete reference sets and payloads before restore publication. Preserve
  old manifests, rollback new leases on failure, and clean only unreferenced payloads.
- [x] Test actual encrypted payloads, missing/tampered assets, source-file deletion,
  shared history, failed restore with preexisting owners and protected workspaces.
- [x] Run affected native/frontend suites and app build; focused review and material
  fixes. Record evidence and remaining gaps before the placement/properties UI phase.

Do not call this a completed image feature until UI placement, preview agreement,
properties, keyboard/undo behavior, real Windows save/reopen/recovery and portable
checkpoint acceptance also pass. The Adobe/Foxit parity objective stays active.
