# Editable multi-document automation

Extend the qualified single-document page-organization component so local scripts
and MCP clients can combine PDFs and continue editing the result. Reuse the native
nonflattened export engine: it already preserves supported tags, navigation,
languages, metadata, forms and overlays, including collision-safe field names.
The standing broad quality goal remains active. This design is an implementer
decision under the user's authorization to proceed without routine questions.

## Approach and existing policy

One shared composition module builds source-owned PagePlans and calls
`PdfEngine::export_pdf`. Do not implement graph copying in CLI/MCP. A low-level
page importer would duplicate preservation and security logic; an interactive
preview format adds no useful decision to an explicit page map. Existing text
and form preview tools remain available on the adopted result.

Native form namespace policy keeps independent source fields independent,
renaming later colliding roots with the existing deterministic suffixes. Repeated
pages from the same logical source retain shared answers and separate widgets.
Do not rename fields in the transport or promise fixed names before inspection.
Native metadata, bookmarks, destinations, language, permission and graph-refusal
policies remain authoritative. Document metadata comes from the first output
source, not necessarily the first configured input. Unsupported preservation
arrangements fail before public output or session adoption.

The engine permits plain plus protected sources and retains the selected protected
source's encryption. Protected sources with incompatible encryption contexts
remain refused; passwords being equal does not make their contexts equal. Do not
silently remove/change security or introduce owner-password replacement here.
All selected protected sources must authorize assembly. Exact single-source
identity requests may preserve original bytes under the established identity rule.

## CLI and typed requests

Add `compose-pages --input A.pdf --input B.pdf [--input ...] --out NEW.pdf
[--request PLAN.json] [--passwords-stdin] [--engine PATH]`. Accept1..64 native OS
input paths; preserve non-Unicode filenames by keeping paths outside JSON. One
input is valid and shares existing page-organization semantics. Missing/duplicate
singleton options, unrelated options and excess inputs fail with argument exit2.

Optional regular UTF-8 PLAN.json, at most512KiB:

```json
{"version":1,"pages":[{"source":2,"page":3,"rotation":90},{"source":1,"page":1},{"source":2,"page":3}]}
```

Source/page indices are1-based. Rotation defaults0, additional clockwise
0/90/180/270. Omission concatenates all pages in configured input order. Explicit
plans contain1..10000 occurrences, selecting source1..64/page1..10000 within opened
source bounds. Reject unknown fields, nulls, fractional indices, invalid versions,
empty maps and out-of-range sources/pages before publication. Repeated paths or
same session document IDs denote one logical source; reuse their immutable
snapshot/opened native identity so duplicate widgets do not become independent
form fields. Distinct configured files remain independent even with identical
bytes; use canonical OS file identity to recognize aliases of the same file.

Capture immutable original source snapshots before native opening, with a shared
512MiB total input budget, not512MiB per source. Reject nonregular/empty/growing
inputs. Keep sources open until export completes; close all on every failure.
At most10000 total input pages and10000 output pages; candidate at most512MiB.
Use source dimensions/rotation/crop/UserUnit/overlays, unique output occurrence IDs,
and `flatten:false`. An effective one-source exact identity copies that captured
source's original bytes, including ciphertext. Otherwise reopen the complete
candidate using its output protection credential before publication and compare
requested page count. No credential enters a result.

Reuse exclusive same-filesystem staging and new `.pdf` publication. Existing
destinations/aliases and late-created targets remain untouched. Reconstruct
`path`/lossless `pathNative`,bytes,sha256,pageCount; add a bounded internal
`protectionSource` index or null indicating which configured credential reopens
the output. This identifies an authorized input slot, never a password. Validate
the512KiB result and complete1MiB CLI envelope including newline before retention.
Keep existing stdout/stderr/exit behavior and single-source commands compatible.

## Credentials and lifetime

CLI `--passwords-stdin` reads at most128KiB of UTF-8 JSON, an array exactly matching
the number/order of repeated input slots, e.g. `[null,"reader"]`. Entries are null
or strings of at most1024 UTF-8 bytes without NUL/CR/LF. Empty string is an explicit
empty password. No flag means all null. Reject wrong shape/count/limits with generic
errors before native work; never echo argument contents, credential values or
parser excerpts. Aliased input slots must have identical credentials or refuse;
do not choose one silently. Native authentication errors preserve established
password-required/incorrect-password distinctions without echoing secrets.

MCP protocol stdin must remain protocol data. Add optional launch-only
`--credentials FILE`, a regular bounded file containing the same array aligned
with launch `--input` order. Read once into session-owned secret buffers; the user
owns that file and Folio neither copies it into export folders nor modifies it.
Tools cannot read/change credentials, paths or encryption settings. No prompting
or network credential exchange. Existing launches without the option work as
before. Owned plaintext buffers use the existing Sensitive zeroing wrapper;
do not claim allocator-wide or crash-dump secrecy.

Each Document carries an optional shared secret capability. Existing Info,
inspection, render, text export, text/paragraph/form previews and organization
workers pass its password through the existing single-password stdin flag.
Preview records retain a private credential capability after original ID closure;
Apply transfers it to the derived document. Save copies the already encrypted
candidate. Merge adopts the credential indicated by its validated authorized
protectionSource slot; plain results carry none. Never serialize capabilities in
document lists, preview lists, errors, traces or output JSON.

Parent worker stdin uses an owned feeder thread with bounded secret payload and
ChildStdin, so even a blocked or exited child cannot block protocol cancellation.
On failure/cancel/EOF reap the child, join the feeder and release its secret buffer
before dropping owned directories. Feeder errors fail the operation before
adoption/retention. Existing one-worker/120-second deadline remains. No plaintext
credential files in worker stages, manifests, results or public exports.

## MCP composition

Add `folio_merge_documents` (new private document) and
`folio_export_composition` (new public PDF). Strict arguments:

```json
{"documentIds":["document-1","document-2"],"pages":[{"source":2,"page":1},{"source":1,"page":2,"rotation":90}]}
```

Require1..64 authorized IDs, optional nonnull/nonempty page map with no top-level
version. Omission concatenates. Reject unknown properties or arbitrary paths.
Repetition denotes the same logical source. Parent maps authorized snapshots to
native repeated --input arguments and owns request JSON, filenames and folders.
Use the same CLI operation; validate returned proof, pageCount and protectionSource
before retention. Reconstruct all summaries; child paths never authorize cleanup.
Remove owned request files and reject unexpected stage contents.

Merge result includes documentId,name,bytes,sha256,pageCount. Export includes
path/pathNative,bytes,sha256,pageCount. Do not expose protectionSource in MCP tool
results; it is internal credential routing. Merge preflights64document capacity
and checks512MiB aggregate again on adoption; rejection does not consume an ID.
Public export creates no document and leaves only its PDF in a unique owned child
of configured output. Source IDs/previews remain immutable. Close/EOF removes only
owned private snapshots; successful public files survive. Verify the candidate
on the same held-file boundary introduced by page organization.

Use2MiB complete new-tool response caps including both representations, metadata,
ID and newline, before keeping output or state. Preserve512KiB input frames and
16MiB worker stdout. Both tool annotations use readOnlyHint:false,
destructiveHint:false,idempotentHint:false,openWorldHint:false. Legacy and modern
protocols, busy/cancel/timeout, repeated exports, existing Apply/Save and close
semantics remain compatible.

## Evidence and delivery

Public authored multi-document fixtures must distinguish source text/ink, metadata,
language, links/bookmarks/tags, colliding independent form fields and same-source
repeated widgets. Include nonunit physical geometry and saved editable overlays.
Test both implicit concatenation and interleaved duplicate/rotated selections,
same-file aliases versus distinct identical files, valid/invalid credentials,
permissions, compatible/incompatible encryption and source/destination preservation.
Exercise protected MCP inspect/preview/apply/save/re-edit after original closure,
credential absence in retained/exported data and feeder cancellation/reaping.

Independent MuPDF/pypdf checks compare full text, raster outside a fixed source edit
corridor, geometry, fields/values/widgets, nonempty tags, metadata and navigation.
Include link rectangles/destination coordinates and actual corruption controls,
addressing the prior navigation evidence gap. Exercise a valid near-512KiB request
ID on the actual protocol path, not merely a large serialized ID unit test.

Use TDD inline with one design builder check and one final fresh scoped review.
Run full library correctness with Folio opt0/release dependencies, affected ordinary
optimized automation integrations, ordinary optimized default CLI/GUI with26bound
resources, exact-binary CLI/MCP and isolated Windows edit/save/reopen acceptance.
Record profile meanings, skips and all failed attempts. No broad frontend/all-target
or portable claim without its own evidence. Preserve selected0.49, prior packages,
the current worktree, private quiz, startup refactor and all original evidence.
No commit/merge/push, cleanup, normal user app launch or release/parity declaration.
