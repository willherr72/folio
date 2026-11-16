# Page organization through CLI and MCP

Folio's desktop already selects, reorders, duplicates and rotates pages using a
native export path that preserves supported forms, navigation, metadata, tags
and editable overlays. Automation currently lacks PDF page output. Extend that
same path so local scripts and MCP clients can organize an existing document,
continue editing it and save a copy without recreating PDF logic in a transport.
This advances the standing broader product goal; it does not establish parity.

The user has authorized autonomous implementation and opted out of routine design
approval questions. Design choices below are the implementer's decisions.

## Approach

Use `PdfEngine::export_pdf` with `flatten:false` and source-derived `PagePlan`s.
Alternative low-level page copying would duplicate the existing graph/permission
rules. A new visual-preview format is unnecessary for an explicit page order;
existing text/form previews remain available after adopting an organized copy.
This component handles one existing document. Multi-input merge is subsequent
work with distinct per-source password and naming decisions, not part of this
operation. No page geometry, font or renderer algorithm changes are planned.

## CLI and native operation

Add `export-pages INPUT.pdf --out NEW.pdf [--request PAGES.json] [--engine PATH]
[--password-stdin]`. Omit `--request` to save an exact full-document copy.
An explicit JSON file has exactly this shape:

```json
{"version":1,"pages":[{"page":3,"rotation":90},{"page":1},{"page":3,"rotation":270}]}
```

Pages are 1-based, in exact output order. Repetitions deliberately duplicate a
page. `rotation` defaults to0 and is an additional clockwise0/90/180/270 degrees
relative to the source's displayed orientation. Retain intrinsic rotation,
crop, UserUnit, original overlays and content through native export. Each output
occurrence gets a unique page-plan ID; clients never supply source IDs, dimensions,
crop rectangles or overlays. Select1..10000 output pages from an input with
1..10000 pages. Reject zero, out-of-source-range, fractional, excessive and empty
selections, invalid rotations, unsupported versions, null entries and unknown
properties. Request files are regular files capped at512KiB, read with a streaming
limit before native PDF work. No request is different from an explicit empty/null
request. Native transport-independent types own these rules.

Capture one immutable bounded original snapshot (including ciphertext) before
opening; maximum512MiB, matching existing automation. For no request or an exact
identity page list with zero extra rotations, copy those original bytes exactly.
Otherwise use the source geometry and existing export. Preserve encryption and
rights; native assembly permissions govern changed page plans. No flattening or
silent security removal. Unsafe/unsupported graph arrangements may be refused
by the existing engine, with no published output; do not weaken those checks.

The destination must be a new `.pdf` in an existing directory. Stage privately
on that filesystem, bound and hash the complete candidate to512MiB, then publish
with an exclusive no-clobber primitive. Existing files, hard-link aliases and
late-created targets remain unchanged. Build the bounded result before publication:
`path`/lossless `pathNative`, `bytes`, lowercase SHA256 and `pageCount`. Do not echo
the whole page map or document contents. A512KiB result budget plus a1MiB final
CLI-envelope limit covers native paths; check before publication. Stdout remains
one success JSON/newline; existing exit/password/error conventions apply.

## MCP operations and ownership

Add two tools with the same schema: required `documentId`, optional `pages` array
of the page/rotation objects above (no `version` exposed). Omission means all pages;
explicit null/empty is invalid. Authorized document IDs only; no tool-supplied
input/output paths, fonts or passwords. Serialize an explicit selection through a
server-owned request file for the same CLI worker operation.

- `folio_organize_pages` creates a new private immutable document ID for continued
  inspection and text/form editing. Earlier IDs/previews/originals remain intact.
  It publishes no public PDF. Use existing64-document/512MiB aggregate capacity,
  monotonic IDs and close/EOF ownership. Preflight a full session; unknown final
  size is checked again before adoption. A capacity failure cleans the candidate.
- `folio_export_pages` saves a fresh PDF under a unique owned child of the configured
  output directory. It creates no session document. Clients can omit `pages` on an
  organized document ID to save its exact immutable bytes. Repeated exports get
  distinct paths. Keep only the output PDF in the successful public export folder.

Both tools use the existing single active worker,120-second deadline and
kill/reap-before-folder-drop cancellation. The former stages privately; the latter
keeps its owned output folder only after success, identity and response validation.
Failure, cancellation and rejected responses discard owned temporary work and
retain sources/session state. Never delete or overwrite arbitrary paths returned
by a child. Derive output paths from the parent's owned folder, reconstruct bounded
summary fields and verify candidate proof rather than trusting returned paths.

Strengthen the shared derived-snapshot adoption boundary: stream-check the opened
candidate's actual bytes/SHA against its declared proof, and keep a Windows
read-shared handle that denies mutation until document closure. Hash the same
held file; existing preview application must continue to work. This is needed
because an organized candidate has no pre-existing preview manifest.

Both tools advertise readOnlyHint:false, destructiveHint:false,
idempotentHint:false and openWorldHint:false. Existing512KiB input frames allow
large request IDs. Preserve them and bound the new tools' complete MCP response
(both representations, metadata, ID and newline) to2MiB; no global ID-policy
change is needed. New summary payloads are at most512KiB. Reject oversized final
responses before keeping exports/adopting state; error envelopes also fit the
cap because their IDs came from bounded input frames. Existing tools keep their
current response limits and behavior.

## Evidence and delivery

Use public authored multi-page fixtures with distinguishable complete text/ink,
intrinsic rotation/crop/UserUnit, links, bookmarks, nonempty tags, form values and
separate saved Folio overlays. Compare native expected page plans and independent
MuPDF/pypdf output order, dimensions, whole content, rotation, graph ownership,
form values and renderer pixels. Test duplicate internal-destination occurrence
rules against existing native composition rules, not merely a valid target.
Exercise no-op byte identity, protected assembly allowed/denied, invalid requests,
existing/late-created/non-Unicode targets and unchanged source bytes.

Run real normal optimized CLI/MCP chains: organize, inspect new ID, edit a selected
run, save, close, and confirm original remains unchanged. Test legacy and modern
protocol, schema hints, unauthorized IDs/paths, full-session capacity, wrong
candidate hashes/same-size corruption, cancellation/EOF/worker failure cleanup,
exact response limits and saved-output confinement. Keep all failed evidence.

Test first, execute inline, one final fresh scoped implementation/evidence review.
Run focused library and affected automation/CLI/MCP integration suites, ordinary
optimized default CLI and GUI builds, real saved-output audits and GUI smoke of
an organized result. Prior complete source/renderer checks are not a claim of a
fresh whole-project run. Preserve the selected0.49 package/launcher, existing
development build-boundary refactor, private quiz and dirty worktree. Batch this
feature with later meaningful work before separate portable qualification; no
commit, cleanup, public release, normal-user-app launch or parity claim.
