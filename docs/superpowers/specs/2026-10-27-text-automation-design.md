# Validated existing-text automation

Build on verified inspection so CLI/MCP users can select a source run, preview a
replacement, save the exact validated PDF as a new copy, or discard the preview.
User delegates implementation choices. Reuse the desktop complete-source native
transaction; no replacement engine, whiteout overlays or loosened fidelity policy.

## Chosen approach

Retain immutable preview bundles on disk and publish their exact bytes. Re-running
the edit during Save would make Save a different transaction from the preview.
Keeping a persistent native engine would complicate cross-worker identity and
resource ownership. Disposable existing CLI workers remain the execution boundary.

CLI:
- `preview-text INPUT.pdf --request EDIT.json --out-dir EXISTING [--engine PATH]
  [--password-stdin]`. EDIT is strict UTF-8 JSON, <=32KiB, with exactly page,
  selectionId, replacement. Page1..10000; selection `text-v1-` +64lowercase hex;
  replacement nonempty, <=4096UTF8 bytes. Native glyph/one-line rules still decide
  admission. Request-file contents are never echoed on errors or command lines.
- The engine re-lists the page, regenerates selectors, and resolves exactly one
  current run. It uses native text as expectedText and invokes
  replace_text_occurrence with the complete original page order and no substitution.
  Source and candidate document registrations are RAII-owned through failures.
- Create a new unique `Folio preview-...` bundle below the explicitly supplied
  parent, containing candidate.pdf, before.png, after.png and manifest.json. Use
  native full-source export with original protection; never raw decrypted bytes.
  Reopen the exported candidate with the supplied credentials for its after image.
  Images use desktop render_page_plan at requested width1200 and its existing
  dimension limits. Each PNG<=32MiB; PDF<=512MiB; total bundle<=512MiB.
- Manifest v1 contains source fingerprint/page/selection/old and replacement text,
  file byte counts and SHA256s. Return bundleName, folder/before/after/candidate
  paths (native-path JSON when needed) and manifestSha256. No user-original changes.
- `save-text-preview BUNDLE --manifest-sha256 HASH --out NEW.pdf [--engine PATH]`
  checks the provided manifest hash, strict bounded manifest and fixed file names,
  verifies both preview PNG hashes and streams candidate bytes into a same-parent
  temporary file while validating its hash/count. Flush and publish exclusively;
  existing files or aliases are never overwritten. No password needed to copy
  already-encrypted candidate bytes. Saving requires the unchanged preview bundle,
  not the original PDF. The hash is a byte-integrity check, not a signature or a
  capability authorizing arbitrary files; CLI paths are explicitly user-authorized.

MCP:
- Add folio_preview_text {documentId,page,selectionId,replacement},
  folio_save_preview {previewId}, folio_discard_preview {previewId}; closed schemas.
- Preview worker writes EDIT privately and uses only the launch snapshot. Adopt
  only a bundle created beneath that worker's owned operation directory, with a
  single safe basename, strict manifest hash and verified fixed file lengths.
  The supervisor reads only the bounded manifest and filesystem metadata; the
  native worker produces validated bytes, and Save rehashes all files inside
  its supervised worker. This keeps large-file hashing off the dispatcher. Return a
  generated opaque previewId, before/after paths and change summary; withhold
  candidate/bundle filesystem paths from the tool result.
- Include retained previews in folio_list_documents so lost result IDs can be
  recovered without exposing arbitrary paths or adding another tool.
- Max8 retained previews and512MiB total. Expire after15minutes idle lifetime;
  pruning waits while a worker is active. Refuse new previews at capacity rather
  than silently discarding ones a person is reviewing. No saved output on failure.
- Save uses a fresh CLI worker with the server-held bundle path and original
  manifest hash, and a new `Folio export-.../Edited.pdf` destination below launch
  output. Exact candidate bytes are published; repeated saves create new copies.
  The preview stays usable until discarded/expired/shutdown. A saved file is not
  automatically registered as a new input; explicit launch inputs remain the
  document boundary for this version.
- Discard deletes only owned private state. All native and save operations use
  the existing one-worker busy/deadline/cancel/EOF supervision. Drop/reap the
  worker before cleaning preview storage. Prevent expiration/discard while a save
  is consuming a preview. Server shutdown cleans retained previews normally.
- General forced-server/process-tree cleanup remains an existing limitation;
  do not claim an OS sandbox against other same-user processes. No model paths,
  password arguments or extra native capabilities are accepted by these tools.

## Acceptance and limits

Test actual CLI inspect→preview→save→reopen, exact original/candidate hashes,
duplicate words/pages, stale selectors, immutable neighboring text/other-page
pixels, bookmarks/annotations, unsupported glyphs and overlap, request limits,
tampered manifest/PDF/images, overwrite/alias refusal, repeated saves, source
deletion after preview, and encrypted allowed/denied user rights with retained
credentials. MCP tests use real workers and assert ID ownership, closed paths,
capacity/discard/expiry, busy/cancel/EOF cleanup, no public export before Save,
and byte-exact saved PDFs. Package and test public Word inputs with the official
SDK legacy/modern; use an independent reader for saved text/geometry.

Initial automated replacement is one supported top-level text run, preserving its
font and location. Paragraph reflow, nested-form editing and substitution remain
native desktop features to expose in subsequent automation milestones. Do not
claim those capabilities here or commercial parity. Preserve all prior work,
originals, apps, artifacts; no commits/releases/cleanup. One Cargo/build at a time.
