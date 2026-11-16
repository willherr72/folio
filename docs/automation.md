# Folio automation

## Atomic text batches in development

Newer development source adds `preview-text-batch` and
`folio_preview_text_batch`. These operations use a newer development candidate;
the completed 0.52 portable and selected 0.50 executable do not include them.
They prepare one candidate from explicit inspected selections across a document.
Automatic document-wide Find/Replace planning and its desktop controls are later
work.

Inspect every target in the original PDF, then put its source-bound selection ID
in a regular UTF-8 JSON request:

```json
{"version":1,"edits":[{"page":1,"selectionId":"text-v1-<inspection hash>","replacement":"First change"},{"page":2,"selectionId":"text-v1-<inspection hash>","replacement":"Second change"}]}
```

```powershell
.\folio-cli.exe preview-text-batch Original.pdf --request edits.json --out-dir Previews
.\folio-cli.exe save-text-preview "Previews\Folio preview-..." --manifest-sha256 <returned hash> --out Edited.pdf
```

The MCP equivalent uses one authorized document ID:

```json
{"name":"folio_preview_text_batch","arguments":{"documentId":"document-1","version":1,"edits":[{"page":1,"selectionId":"text-v1-<inspection hash>","replacement":"First change"},{"page":2,"selectionId":"text-v1-<inspection hash>","replacement":"Second change"}]}}
```

Root and nested-text IDs may share a batch. All IDs refer to the same original
snapshot; inspect again after Apply to obtain IDs for subsequent edits. Use the
existing `folio_save_preview`, `folio_apply_preview` and `folio_discard_preview`
tools with the returned preview ID. Apply creates a new immutable document ID.
The original document and its bytes remain intact. A late failure publishes no
partial candidate. Native placement, font, graph and collision checks still apply
to every step and may refuse an uncertain edit.

A batch accepts 1–64 unique selections across at most 32 pages, with nonempty
replacements of at most 4,096 UTF-8 bytes each. Requests and version-6 manifests
are capped at 512 KiB. Every affected page has before/after PNGs (32 MiB each);
all images participate in tamper checks and the existing 512 MiB bundle/session
budget. The batch result is capped at 256 KiB, including image paths. MCP retains
the existing eight-preview and 15-minute limits. Older preview versions keep
their existing rules.

Optional `fontId` on an edit must name a font authorized through launch `--font`;
omission retains the original font. Copy and edit permissions are required even
for unchanged text. An entirely unchanged batch without font overrides retains
the exact original candidate bytes. Protected inputs use the existing CLI stdin
and launch-only MCP credential mechanisms; passwords are not tool arguments.

Saving preserves an existing destination rather than overwriting it. Batching
uses the existing faithful edit operations sequentially, so large batches can
take longer than one edit. These limits do not establish support for arbitrary
PDF text shaping or commercial-editor parity.

## Multi-document composition in development

Development source now provides `compose-pages`, `folio_merge_documents` and
`folio_export_composition`. Corrected ordinary optimized CLI/GUI, independent
saved-PDF checks and actual Windows editing qualification passed on 2026-11-13.
The selected **0.50 portable includes these operations**, as does the separate
reviewed 0.52 candidate. See the [current build status](development-build-status.md).

The single final review identified two Important defects. One TDD fix pass
corrected zeroing ownership on credential parse/escape failures and rejection of
observed source-size changes during held-file capture. Fresh checks passed:
1,596 library tests / 11 ignored (Folio opt0, release dependencies), 120 tests in
11 ordinary optimized integration targets, 15 independently checked saved PDFs
and 16 actual corruption controls. Exact-candidate protected CLI/MCP chaining,
512KiB requests, native Windows paths, and GUI fill/text-edit/save/reopen passed.
[Component evidence and rulings](../artifacts/multi-document-automation-20261113/component-completion.json).

This qualifies the authored Windows corpus, with native unsupported-graph,
mixed-form-language and incompatible-encryption refusals retained. It is not a
fresh whole-frontend/every-integration run or portable/release/parity approval.
Owned-buffer zeroing is not process-wide memory secrecy; observed-length checks
are not atomic filesystem snapshots or same-size-write locking. Launch-only credentials below apply to protected inspection and editing;
interactive or tool-supplied password entry remains unsupported.

```powershell
.\folio-cli.exe compose-pages --input A.pdf --input B.pdf --out Combined.pdf
.\folio-cli.exe compose-pages --input A.pdf --input B.pdf --request plan.json --out Selected.pdf
```

Omitting `--request` concatenates all pages in input order. A regular UTF-8 JSON
plan (at most512KiB) selects, interleaves, duplicates and rotates pages:

```json
{"version":1,"pages":[{"source":2,"page":3,"rotation":90},{"source":1,"page":1},{"source":2,"page":3}]}
```

`source` and `page` are1-based. Rotation is an additional clockwise0/90/180/270
degrees; omission means0. Supply1..64 native input paths and1..10000 output
occurrences, with at most10000 unique source pages and512MiB captured source
bytes. Each candidate is capped at512MiB. Empty/null maps, unknown fields,
invalid indices/rotations and unsupported versions are rejected. Aliased paths
reuse one logical snapshot; independent files remain independent even when their
bytes match. Aliased slots must have identical credentials.

Output must be a new `.pdf` in an existing directory; existing files, aliases and
late-created targets are preserved. The native engine retains supported forms,
links/bookmarks, tags, languages, metadata and editable overlays. Independent
colliding form names receive native suffixes; repeated pages of one logical source
share answers. Inspect the result to obtain current field names and source-bound
selection IDs. Document metadata follows the first output source.

Native preservation and permission checks remain active. Protected inputs must
permit assembly for changed plans; their encryption is retained. Different
protected encryption contexts are refused, even when opening passwords match.
Mixed document-language defaults on forms retain the native refusal. Effective
single-source identity requests copy that source's original bytes exactly.
There is no automatic flattening or security removal.

For single-document commands, `--password-stdin` reads one password line. For
composition, `--passwords-stdin` reads UTF-8 JSON aligned with the repeated input
slots, at most128KiB total:

```json
[null,"opening-password-for-B"]
```

Entries are null or strings of at most1024 UTF-8 bytes without NUL/CR/LF; an empty
string is an explicit empty password. The array length must exactly match input
slots. Password values never go in command arguments or results. For example,
Python can prompt and provide UTF-8 stdin without placing the value in arguments:

```python
import getpass, json, subprocess
password = getpass.getpass("Opening password for B: ")
subprocess.run([
    r".\folio-cli.exe", "compose-pages", "--input", "A.pdf", "--input", "B.pdf",
    "--out", "Combined.pdf", "--passwords-stdin",
], input=json.dumps([None, password], ensure_ascii=False).encode("utf-8"), check=True)
```

CLI success includes `path`/lossless `pathNative`,bytes,sha256,pageCount and nullable
`protectionSource` (the1-based credential slot for reopening the result). It never
contains the credential itself. Result and complete CLI response limits are512KiB
and1MiB, including the final newline.

MCP accepts optional launch-only `--credentials FILE`, a regular bounded file with
that same array aligned to launch `--input` order. Folio reads it once and leaves
the user-owned file unchanged. Credentials remain private session capabilities;
workers receive them through owned stdin pipes, not files or tool arguments.
Protected inspection/previews/Apply/Save and later edits retain the capability,
including when a preview outlives its original document ID.

```powershell
.\folio-cli.exe mcp --input A.pdf --input B.pdf --credentials credentials.json --out-dir Exports
```

```json
{"name":"folio_merge_documents","arguments":{"documentIds":["document-1","document-2"],"pages":[{"source":2,"page":1},{"source":1,"page":2,"rotation":90}]}}
{"name":"folio_export_composition","arguments":{"documentIds":["document-3"]}}
```

Use the returned ID. Both tools require1..64 launch-authorized/derived IDs and
optional `pages` without a top-level version; omission concatenates. Repeated IDs
refer to the same logical source. `folio_merge_documents` creates a new private
immutable ID for continued editing; `folio_export_composition` saves a new PDF in
a unique configured output child and creates no document. Earlier IDs stay intact.
The internal credential-routing index is omitted from MCP results.

Close unused IDs to recover the existing64-document/512MiB session capacity.
Successful exports survive close/EOF; failures and cancellation discard only owned
private work after reaping the worker and joining stdin delivery. The single-worker
120-second deadline remains. New complete responses are capped at2MiB including
both protocol representations, metadata, ID and newline. Input frames remain512KiB.


## Page organization in development

The operations below are qualified in a local Windows development candidate.
The selected **0.50 portable includes them**, as does the separate reviewed 0.52
candidate. The [component completion](../artifacts/page-organization-automation-20261113/component-completion.json)
binds the exact CLI/GUI candidate and its scope; [review limitations](../artifacts/page-organization-automation-20261113/review/review.md)
record the deferred navigation-geometry and maximum-frame evidence gaps.

`export-pages` saves a new editable PDF through the desktop export engine. Omit
`--request` for an exact byte-for-byte full-document copy, including its existing
encryption. To select, reorder, duplicate or rotate pages, provide a UTF-8 file:

```json
{"version":1,"pages":[{"page":3,"rotation":90},{"page":1},{"page":3,"rotation":270}]}
```

```powershell
.\folio-cli.exe export-pages "C:\Documents\Report.pdf" --request "C:\Documents\pages.json" --out "C:\Exports\Organized.pdf"
.\folio-cli.exe export-pages "C:\Documents\Report.pdf" --out "C:\Exports\Exact copy.pdf"
```

Page numbers start at 1. Output order follows the array exactly; repeated numbers
make separate page occurrences. Rotation is additional clockwise rotation relative
to the source display, and must be 0, 90, 180 or 270; omission means 0. An explicit
request requires version 1 and 1–10,000 page entries. Empty/null selections, unknown
fields, fractional/out-of-range pages and invalid rotations are rejected. Request
files are capped at 512 KiB. Inputs and candidates are capped at 512 MiB. The result
returns `path` or lossless `pathNative`, `bytes`, `sha256` and `pageCount`.

The destination must be a new `.pdf` in an existing folder. Existing files and
aliases are preserved, including targets created while export is in progress.
Changed plans retain supported forms, links, bookmarks, tags, metadata and editable
Folio overlays through the native export path. Native preservation and assembly
permission checks still apply; unsupported arrangements can be refused. CLI
passwords use the existing `--password-stdin` option. There is no implicit flattening,
font substitution or removal of security.

MCP provides two tools, each with `documentId` and optional `pages` using the same
page/rotation entries (without a top-level `version`):

- `folio_organize_pages` creates a private immutable `documentId` for continued
  inspection/editing. It leaves earlier IDs intact and saves no public file.
- `folio_export_pages` saves a new PDF in a unique child of the configured output
  directory. It creates no session document. Omit `pages` on an organized ID to
  save its exact bytes; repeated exports create distinct paths.

```json
{"name":"folio_organize_pages","arguments":{"documentId":"document-1","pages":[{"page":2,"rotation":90},{"page":1},{"page":2}]}}
{"name":"folio_export_pages","arguments":{"documentId":"document-2"}}
```

Use the ID actually returned by the first call. Organized documents share the
existing limit of 64 live documents and 512 MiB total; close unused IDs to recover
capacity. Closing a private ID leaves exported files and earlier documents intact.
Only launch-authorized/derived IDs are addressable; tools accept no arbitrary file
paths or passwords. Existing single-worker, 120-second cancellation/EOF cleanup
rules apply. Large valid documents can exceed the deadline and be refused.
Complete new-tool MCP responses are capped at 2 MiB, including both representations,
metadata, request ID and newline; CLI responses are capped at 1 MiB. Originals
remain unchanged. These operations organize one document; multi-input composition
is subsequent work.



The Windows `folio-cli` console uses the same native engine as the desktop app.

The combined portable development package includes `folio-cli.exe` beside `Folio.exe`.
Keep the shared `resources` folder in place. Run the console commands below from
PowerShell in the extracted package, or use the executable's absolute path.

This version reads saved PDFs, inspects existing text and forms, previews text edits and filled answers,

saves validated PDF copies, exports text, and renders PNGs. It does not attach to

open desktop tabs. The same executable provides these workflows through local MCP.



## Commands



```powershell

.\folio-cli.exe --help

.\folio-cli.exe info "C:\Documents\Report.pdf"

.\folio-cli.exe inspect-text "C:\Documents\Report.pdf" --page 1

.\folio-cli.exe inspect-nested-text "C:\Documents\Report.pdf" --page 1

.\folio-cli.exe inspect-paragraph "C:\Documents\Report.pdf" --page 1 --selection-id "text-v1-<hash from inspect-text>"

.\folio-cli.exe text "C:\Documents\Report.pdf" --out "C:\Exports\Report.txt"

.\folio-cli.exe text "C:\Documents\Report.pdf" --out "C:\Exports\Page2.txt" --page 2 --include-form-values

.\folio-cli.exe render "C:\Documents\Report.pdf" --out-dir "C:\Exports" --page 2 --dpi 150

```



Page numbers start at 1. Omitting `--page` exports all pages (up to 10,000) for

text/render; inspection requires a page.

Text is the desktop UTF-8 export, including a BOM and form-feed separators between

pages. `--include-form-values` adds supported visible field answers; password fields

are omitted. Rendering defaults to 150 DPI; 72, 150, 300 and 600 are accepted.

The engine's existing image dimensions and export budgets still apply.



Text output must be a new `.txt` file in an existing directory. Existing files,

including aliases of the input, are never overwritten. Rendering creates a uniquely

named `Folio images-...` child folder under the specified existing directory, with

names such as `page-0002.png`. The JSON result gives the actual output location.

Source PDFs remain unchanged. Saved Folio annotations and OCR use the same export

paths as desktop; this is not a raw PDFium extraction wrapper.



## JSON and exit status



Successful operations write one JSON object to stdout:



```json

{"schemaVersion":1,"ok":true,"command":"info","result":{"name":"Report.pdf","pageCount":1,"pages":[{"page":1,"width":612.0,"height":792.0}],"protection":null}}

```



Dimensions are physical PDF points (72 per inch), in the displayed page orientation.

No internal engine source identifiers appear in the result.

Output `path` and `folder` are Unicode strings when representable. For native

non-Unicode filenames the field is `null`, and a sibling `pathNative` or

`folderNative` carries a lossless representation: `{"encoding":"windows-utf16",

"units":[...]}` on Windows, or `{"encoding":"unix-bytes","bytes":[...]}` on

Unix. Reconstruct native paths from those units; do not replace them with a lossy

display string. This also applies to native parent directory names.

Errors write one JSON object to stderr with `ok:false` and `error.code`/`error.message`.

Exit status is 0 for success, 2 for invalid arguments/password input, or 1 for an

operation failure. Codes include `password_required`, `incorrect_password`,

`permission_denied`, `engine_unavailable`, `invalid_request`, `invalid_document`,

`unsupported_protection`, `limit_exceeded`, `engine_stopped` and `io_error`.

`--help` and `--version` print plain text without loading the engine.



## Passwords and renderer



Use `--password-stdin` to supply one UTF-8 line from a private input stream. No

password command-line argument is supported. At most 1024 password bytes are

accepted; one terminal LF or CRLF is removed, other line breaks and NUL are refused.

The CLI does not echo or retain the password. PDF permissions are enforced by the

same native policy used in desktop exports.



By default PDFium is loaded only from `resources/pdfium/pdfium.dll` beside the

executable. `--engine PATH` explicitly selects another compatible renderer. The

release checkpoint must include the exact patched Folio renderer and its notices;

an arbitrary PDFium build is not a supported substitute.



Development build:



```powershell

cargo build --manifest-path src-tauri/Cargo.toml --offline --bin folio-cli

# For an unstaged development executable, supply the repository renderer explicitly:

path\to\folio-cli.exe info "C:\Documents\Report.pdf" --engine src-tauri\resources\pdfium\pdfium.dll

```



`cargo run` still starts the desktop app. The console never launches a WebView.

Paths are those explicitly authorized by the invoking local user. This CLI is not

a remote service, path sandbox, or MCP authorization boundary. Image publication

currently requires Windows, matching the desktop batch exporter.



## MCP connection



Launch the local stdio server with the PDFs it may read and an existing folder

for new exports:



```powershell

.\folio-cli.exe mcp --input "C:\Documents\Report.pdf" --out-dir "C:\Exports"

```



Repeat `--input` to supply more PDFs. A typical MCP client configuration is:



```json

{

  "mcpServers": {

    "folio": {

      "command": "C:\\Tools\\Folio\\folio-cli.exe",

      "args": [

        "mcp", "--input", "C:\\Documents\\Report.pdf",

        "--out-dir", "C:\\Exports"

      ]

    }

  }

}

```



Adjust those paths to your files. Keep the packaged `resources` folder next to

the executable, or add `--engine` and the compatible Folio PDFium path to `args`.

No graphical window opens. The client owns the process and exchanges JSON-RPC

over its standard streams; do not put passwords or conversational text on stdin.



| Tool | Arguments | Result |

| --- | --- | --- |

| `folio_list_documents` | `{}` | Authorized document IDs, names, byte counts and retained previews |

| `folio_info` | `{"documentId":"document-1"}` | Page dimensions and permissions |

| `folio_inspect_text` | `{"documentId":"document-1","page":1,"offset":0}` | Up to 100 existing text runs, stable selections, geometry and eligibility reasons |
| `folio_inspect_nested_text` | `{"documentId":"document-1","page":1,"offset":0}` | Up to 100 nested reusable-object text runs; source-build addition described below |

| `folio_inspect_forms` | `{ "documentId":"document-1", "page":1 }` | Existing fields, current answers, options and stable field IDs; follow nextOffset |

| `folio_preview_form` | `{ "documentId":"document-1", "page":1, "fieldId":"field-v1-…", "value":{"kind":"text","text":"Answer"} }` | Native fill preview and preview ID for Apply/Save |

| `folio_inspect_paragraph` | `{"documentId":"document-1","page":1,"selectionId":"text-v1-…"}` | Native paragraph suggestion, mixed styles, geometry and a stable paragraph ID |

| `folio_export_text` | `{"documentId":"document-1","page":1,"includeFormValues":false}` | New text file, excerpt, truncation flag and export counts |

| `folio_render_page` | `{"documentId":"document-1","page":1,"dpi":150}` | New folder containing `page-0001.png` |

| `folio_preview_text` | `{"documentId":"document-1","page":1,"selectionId":"text-v1-…","replacement":"Updated text"}` | Preview ID, before/after PNG paths, change summary and candidate hash |

| `folio_preview_paragraph` | `{"documentId":"document-1","page":1,"seedSelectionId":"text-v1-…","paragraphId":"paragraph-v1-…","runs":[{"text":"Updated paragraph","styleId":0}],"fit":true}` | Native reflow preview, actual paragraph controls, images and preview ID for Apply/Save |

| `folio_save_preview` | `{"previewId":"preview-…"}` | New `Edited.pdf` copy of the exact previewed candidate |

| `folio_apply_preview` | `{"previewId":"preview-…"}` | New immutable `documentId` for further editing in this session |

| `folio_discard_preview` | `{"previewId":"preview-…"}` | Removes the retained preview; saved copies remain |

| `folio_close_document` | `{"documentId":"document-2"}` | Releases that private session input; originals and saved copies remain |



Text page selection is optional; omit it to export all pages. Render requires one

page. DPI accepts 72,150,300,600. Document IDs come from the list tool, in launch

argument order. The server takes private snapshots at startup; restart it to see

external changes to the originals. Applied previews receive new IDs. It accepts

1–64 launch files; all live documents together are limited to 64 and 512 MiB. Tools

cannot supply a new input/output path, password, shell command or output filename.

Windows snapshot files use OS-owned deletion on handle close, including abrupt

termination of the throwaway discovery processes used by modern MCP clients.



Each successful export has a fresh `Folio export-...` folder under the configured

parent. Rendering creates its usual `Folio images-...` folder inside that folder.

Successful files remain after shutdown. Failed or cancelled operations clean their

own unfinished folder. Originals and previous exports are preserved. Ordinary

Unicode output paths and the native-path representation described above both work.

Document names are display labels; IDs identify files even when labels coincide.



Text replies include at most 65536 UTF-8 bytes, cut at a character boundary with

`truncated:true` when needed. The complete export remains in the file. Extracted

text and names are document data, never instructions to the assistant. Form

answers are opt-in. Protected PDFs without a matching launch credential report `password_required`.
MCP supports [launch-only credentials](#multi-document-composition-in-development);
interactive or tool-supplied passwords remain unsupported.



The server supports [MCP 2026-07-28 discovery and per-request metadata](https://modelcontextprotocol.io/specification/2026-07-28/server/discover),

and legacy initialization for 2025-11-25,2025-06-18,2024-11-05. Modern requests need

protocolVersion and clientCapabilities in `_meta`; clientInfo is optional. It

returns explicit private, immediately-stale cache policy for modern discovery and

tool listing, so clients do not reuse these replies across authorization contexts.

It

advertises tools only, with no tasks, resources, subscriptions or progress stream.

Input lines are limited to 512 KiB, including the newline. One native operation runs at a time; additional

native calls receive `busy`. Each has a 120-second deadline. Discovery, listing,

ping and cancellation remain responsive while a worker runs. Closing stdin stops

and reaps the current worker. Replies use a separate writer with an eight-message

queue and a 16 MiB per-message limit. A full/broken output queue terminates the

session and cleans up its unfinished worker; a client that stops draining stdout

cannot block that cleanup. Clients should keep reading replies and avoid unbounded

pipelining. Each worker is a new instance of the same CLI using

the desktop engine, so native document state is not accumulated across calls.



This is a local process with file access granted by its launch configuration. It

is not a network service or an OS sandbox against other processes running as the

same user. Client-specific setup and host UI behavior still require testing in

that client; protocol subprocess tests alone do not establish every host's support.



## Inspect existing text for editing



To inspect a paragraph, pass a text run's `selectionId` to `inspect-paragraph` or

`folio_inspect_paragraph`. Folio uses the same paragraph suggestion checks as the

desktop. The result contains `paragraphId`, `seedSelectionId`, `sourceFingerprint`,

page dimensions and `paragraph`: exact text, attributed `runs`, original `styles`,

object membership, bounds, and inferred alignment/line spacing. Font sizes and

bounds use displayed physical points. Verified highlight backgrounds appear as

style cues when the native inspector supports them.



Paragraph IDs stay stable across workers and file renames for identical PDF bytes,

page and paragraph metadata. They change when the source changes. A style ID is

local to that paragraph, not a global font identifier. Both copy and edit rights

are required. Paragraph eligibility is checked independently of one-run replacement

eligibility. Suggestions can fail for unsupported source content; a successful

inspection does not guarantee that a later replacement fits.



Paragraph inspection writes no export or preview files and rejects complete CLI/MCP

responses over256KiB without truncation. `preview-text` still replaces one run under

its existing constraints. For reflow, use the paragraph workflow below.



### Preview and save a paragraph



Inspect the paragraph first, then copy its `seedSelectionId`, `paragraphId` and

attributed `paragraph.runs` into an edit request. Change the text while keeping

the original style IDs where those styles should remain:



```json

{

  "page": 1,

  "seedSelectionId": "text-v1-<inspected hash>",

  "paragraphId": "paragraph-v1-<inspected hash>",

  "runs": [{"text": "A longer paragraph that needs room to wrap.", "styleId": 0}],

  "fit": true

}

```



Use every needed original style in mixed-format paragraphs; replacing the entire

run list with style0 deliberately applies that style to all replacement text.

Style IDs must come from this exact paragraph. Folio rechecks source bytes,

membership and paragraph identity before using the desktop's native reflow and

preservation proofs. Stale selections fail; inspect an applied/saved version again.



```powershell

.\folio-cli.exe preview-paragraph "Report.pdf" --request "edit.json" --out-dir "C:\Exports"

# The historical publisher name supports both text-run and paragraph bundles:

.\folio-cli.exe save-text-preview "C:\Exports\Folio preview-…" --manifest-sha256 "<returned hash>" --out "C:\Exports\Edited.pdf"

```



Optional `bounds` is `{x,y,width,height}` in displayed physical points.

Without explicit bounds, the first source baseline is retained. Changing the area's
width or height also retains that baseline, including when Fit adds headroom above
the text. A bounds-only move with exactly the inspected width and height translates
the baseline by the area's origin offset. Copy the inspected dimensions exactly
when using this position-only convention.

For explicit placement or a combined move and resize, provide
`baselineAnchor: {x,y}` in displayed physical coordinates. It overrides the default
without authorizing different source content. Only its baseline coordinate positions
the line; horizontal placement still follows the area and alignment. On
quarter-turned pages the baseline coordinate is displayed `x` rather than `y`.

To reuse exact fitted placement, copy both `paragraph.bounds` and
`paragraph.baselineAnchor` from the preview. To deliberately move the paragraph,
translate both by the same displayed offset. Preview reports its resolved baseline
when available. An exact unchanged source request preserves the original PDF bytes.

Anchor coordinates must be finite, with absolute values at most 1,000,000.
Null, missing coordinates and extra members are rejected. The native editor still
requires the actual text to fit the page, area and clip without new collisions. With

`fit:false` (the default), text wraps within the specified rectangle or the inspected

source rectangle and fails if it cannot fit safely. With `fit:true`, native fitting

can expand the area without moving unrelated content or shrinking fonts. The result

reports the actual bounds. Omitted alignment/lineSpacing retain inspected values,

using the desktop's left/1.2 defaults when inference is absent. Explicit alignment

accepts left, center, right or justify; lineSpacing ranges from0.6 to3.



Requests allow1–1024 style runs, at most4096 aggregate UTF-8 text bytes, and at most

32KiB serialized edit JSON. Use LF line breaks. Unknown fields, explicit nulls and

caller font/resource IDs are rejected. Source glyph, grapheme, collision, highlight

and permission checks still apply; unsupported PDFs can be refused. CLI passwords

use the same stdin option. MCP uses `folio_preview_paragraph` with `documentId` and

the edit fields, then the existing Apply/Save/Discard tools; originals stay immutable.

Paragraph bundles use manifest v2, and existing text-run v1 bundles remain readable.



```powershell

.\folio-cli.exe inspect-text "C:\Documents\Report.pdf" --page 1

```



The CLI and `folio_inspect_text` use the same native inspection as the desktop

editor. They return top-level text runs with `text`, optional `sourceText`, font

information, `bounds`, `supported`, and reasons when editing is unsupported.

`sourceText`, when present, is the exact PDF operand text; `text` can include

spaces generated by the PDF reader. Neither value is trimmed or normalized.

`supported:true` is preliminary eligibility: a replacement must still pass the

engine's font, placement, overlap and document-preservation checks.



Each run adds an opaque `selectionId`. It is stable across worker restarts and

file renames for identical loaded PDF bytes and native inspection data. Changes

to the source, page or run invalidate it. IDs are not authorization to edit and

should not be constructed or parsed by clients. `sourceFingerprint` identifies

the engine's loaded bytes (decrypted bytes for an unlocked protected PDF).

Native process document IDs and filesystem snapshot names are never returned.



`coordinateSpace` is `displayed-page-points`: bounds start at the page's top-left

and include source rotation, crop and physical page scaling. Page `width` and

`height` use the same points. Page numbers are 1-based and required. Results contain

`totalRuns`, `offset`, `runs`, and `nextOffset`; pass that offset to continue until

it is null. Each response contains at most 100 runs. CLI continuation uses

`--offset N`; MCP uses `offset`. Offsets range from 0 to 100000. The source must

remain identical while paging; compare `sourceFingerprint` across replies.



Inspection requires both copy and edit permissions. CLI `--password-stdin` uses

the existing password contract. MCP uses the configured
[launch-only credential](#multi-document-composition-in-development); tools cannot
supply passwords. No

PDFs are written and no export folders persist. An empty or unsupported page can

return a page-level reason. This tool does not discover OCR or text inside reusable

form objects. Use the returned selection with the preview workflow below.



Each serialized run is limited to 16 KiB and the result to 2 MiB. Oversize data

fails explicitly instead of returning truncated text. Pagination bounds returned

data; native inspection still examines the whole page. The MCP worker retains

its 120-second deadline and cancellation behavior.



## Preview and save existing text



Inspect the page, choose a supported run and copy its complete `selectionId` into

a UTF-8 JSON file. The replacement replaces that entire run, so include all text

you want to keep. The file has exactly these fields:



```json

{"page":1,"selectionId":"text-v1-<64 lowercase hexadecimal characters from inspection>","replacement":"The complete updated text"}

```



```powershell

.\folio-cli.exe preview-text "C:\Documents\Report.pdf" --request "C:\Edits\edit.json" --out-dir "C:\Previews"

.\folio-cli.exe save-text-preview "C:\Previews\Folio preview-<returned name>" --manifest-sha256 "<returned manifestSha256>" --out "C:\Exports\Updated.pdf"

```



Preview creates a new folder with `candidate.pdf`, `before.png`, `after.png` and

`manifest.json`. Review the images before saving. Save verifies the manifest and

all three files, then publishes the exact candidate bytes to a new filename.

Changing any bundle file invalidates it. The original PDF is not needed at Save

time. Keep the returned manifest hash separately; it checks integrity but is not

a digital signature. CLI preview folders remain until you remove them yourself.



The source selection is revalidated against the current PDF. Edits preserve the

selected run's font and placement and must pass the desktop engine's glyph,

overlap and document-preservation checks. Longer replacements can be refused when

they collide with surrounding content. These text-run tools edit one top-level

text run; use the paragraph tools described above for paragraph reflow. Nested

form text and font substitution remain unsupported. Request files are limited to 32 KiB and

replacement text to 1–4096 UTF-8 bytes. The native one-line rules also apply.



For an encrypted source, inspect and preview accept `--password-stdin`. The

candidate retains the source protection, and saving that candidate needs no

password. Output files and original files are never overwritten.



In MCP, call inspect → preview → save or discard using the returned IDs. No tool

accepts an input or output path. The server retains at most eight previews and

512 MiB total, for 15 minutes after creation; expiration waits for any active

worker. Each preview returns an `expiresInSeconds` estimate. Listing documents

also lists retained previews, so their IDs can be recovered after a lost reply.

Creating previews writes no public export. Save creates a fresh `Folio export-…`

folder with `Edited.pdf`, and repeated saves create separate identical copies.

Discard, expiry and normal shutdown remove private preview files. Save results

persist. Preview files contain document content; handle their paths accordingly.

Forced process termination can leave private operation files behind.



An MCP Save does not automatically register another input. Use **Apply preview**

to continue from the same validated bytes, before or after Save:



1. Inspect a document and create a replacement preview.

2. Call `folio_apply_preview` with its `previewId`. The result contains a new

   `documentId`, display name, byte count and SHA256, with no private input path.

3. Inspect that new document ID to obtain fresh selections for the next edit.

4. Save the preview if you need a public PDF copy. Once you have applied it and

   finished saving, call `folio_discard_preview` to release its temporary files.

   Apply and Save both retain the preview; a session holds at most eight previews.

5. Close any earlier document version you no longer need with

   `folio_close_document`. Keep the new applied document ID for the next edit.

6. Inspect the new document, preview its next change, and repeat. Discarding an

   applied preview preserves its new document and any saved copies.



Apply creates an independent private copy and writes no public export. Earlier

document versions remain available; editing a saved file externally cannot change

an applied version. Discarding a preview does not close its applied documents.

Applying the same preview twice creates two document versions and charges both

against the session's 64-document/512 MiB budget. At capacity, Apply fails while

keeping the preview available. Close an unused document to free space, then retry.



`folio_close_document` releases only that ID's private snapshot. It can close launch

inputs too; originals, exports and existing previews remain available. IDs are

never reused or reassigned, even after closing every document. Listing returns

the remaining versions, including versions whose Apply reply was lost. Documents

do not expire automatically; Close or normal server shutdown releases them.

Save edits you want to keep before closing their last document and preview.

Close and Discard advertise that they remove session state; Apply and Save are

additive operations, following the [MCP tool annotation contract](https://modelcontextprotocol.io/specification/2026-07-28/schema#toolannotations). These hints do not change the actual file-access boundary.



Apply, Close, Discard and Save receive `busy` while another native operation runs,

preventing cleanup of files in use. Applying still verifies every preview file;

changed or expired previews must be recreated. Protected inputs use their configured
[launch-only credential](#multi-document-composition-in-development). The private
credential follows the preview and applied document, including after the source
ID closes. Missing or incorrect credentials retain their normal errors; tools
cannot accept password values.



## Text inside reusable PDF objects

The selected 0.50 and separate reviewed 0.52 builds provide `inspect-nested-text` and MCP
`folio_inspect_nested_text`. These inspect text inside reusable PDF Form objects,
which are distinct from fillable form fields. See the
[current build status](development-build-status.md) for candidate identities.

```powershell
.\folio-cli.exe inspect-nested-text "C:\Documents\Report.pdf" --page 1
.\folio-cli.exe inspect-paragraph "C:\Documents\Report.pdf" --page 1 --selection-id "nested-text-v1-<inspected hash>"
```

Ordinary `inspect-text` continues to return page text with unchanged IDs and
ordering. Nested discovery returns at most 100 records per batch. Follow
`nextOffset` until null; offsets are bounded to 100000. Records include exact text,
physical bounds, native eligibility/reasons and a `nested-text-v1-` selection ID.
The complete record, including its ID, is limited to 16 KiB. The final CLI/MCP
response is limited to 2 MiB, including escaping, envelopes and trailing newline.
Oversized records are refused without truncation. Both copying and editing
permission are required. Discovery itself does not create a preview.

Pass a nested selection to the existing `preview-text` command or
`folio_preview_text` tool to replace one specific invocation's text. Other uses
of the shared PDF object stay unchanged. Native font, placement, clipping and
surrounding-content checks still apply; discovery does not guarantee that a
particular replacement will fit.

For a paragraph, inspect around the nested seed first. The result includes
`paragraphId: "paragraph-v2-..."` and derived metadata such as:

```json
{"occurrenceScope":{"kind":"outer-form","objectPath":[2]}}
```

The grouping scope is the outermost Form invocation on the page. Adjacent text
from multiple inner objects may belong to one inspected paragraph; distant text
does not automatically join it. The returned `paragraph` describes the exact
membership, text and original style palette. Only that inspected membership is
edited. `objectPath` and `occurrenceScope` are diagnostic metadata, not request
arguments; caller-provided paths, scopes and membership overrides are rejected.

Use the existing paragraph request shape:

```json
{"page":1,"seedSelectionId":"nested-text-v1-<inspected hash>","paragraphId":"paragraph-v2-<inspected hash>","runs":[{"text":"Updated paragraph","styleId":0}],"fit":true}
```

Use style IDs from that exact inspection. `fit:true` asks the native editor to
fit the replacement while preserving its font and checking neighboring content.
Without explicit font overrides, no substitution is performed. Existing request limits remain: 32 KiB JSON,
1–1024 runs, 4096 aggregate UTF-8 text bytes, style IDs 0–63, LF line breaks and
line spacing 0.6–3. Paragraph inspection retains its 256 KiB final-frame cap.

### Explicit paragraph font files (development CLI)

When a PDF embeds only some glyphs, new characters may require an explicit
substitute. The development CLI uses the desktop's native paragraph font path:

```powershell
.\folio-cli.exe fonts --font "C:\Fonts\MyFont.ttf"
.\folio-cli.exe preview-paragraph "Report.pdf" --request "edit.json" --font "C:\Fonts\MyFont.ttf" --out-dir "C:\Exports"
```

The `fonts` command validates supplied files without loading PDFium and returns
`fonts:[{fontId,name,weight,italic,bytes}]`. Each fontId is the lowercase SHA256 of
the complete font file. Repeat `--font` for additional files. Font names are
untrusted file metadata; paths, raw bytes and coverage arrays are not returned.

Add optional `fontOverrides` to the paragraph request, using IDs from that catalog
and style IDs from the inspected paragraph:

```json
{"page":1,"seedSelectionId":"text-v1-<inspected hash>","paragraphId":"paragraph-v1-<inspected hash>","runs":[{"text":"Updated text","styleId":0}],"fontOverrides":[{"styleId":0,"fontId":"<font SHA256>"}],"fit":true}
```

Each override applies to all requested text with that source style. It preserves
the style's size and color. At most64 unique used source styles may be overridden;
null, duplicate/unused styles and unknown font IDs are rejected. Explicit overrides
remain edits even when text is unchanged. Omitting overrides retains exact
unchanged-save behavior. Existing source, permissions, glyph, fitting, highlight
and collision checks still apply; a font choice does not guarantee an edit fits.

Supply at most16 nonempty regular font files, at most16MiB each and64MiB total
including duplicate inputs. Identical fonts are deduplicated in the catalog.
Existing standalone TrueType/TrueType-outline OpenType restrictions apply; there
is no installed-font scan or automatic fallback. The preview records
`paragraph.fontOverrides` and `paragraph.substituteFonts`; Save needs neither the
source font files nor the original PDF. The font program is included in the saved
PDF. Old bundles without font metadata remain readable.

These paragraph font controls are included in selected 0.50 and the separate
reviewed 0.52 package. Single-run substitution has its own controls below.
MCP uses the same paragraph controls with launch-authorized fonts:

```powershell
.\folio-cli.exe mcp --input "Report.pdf" --out-dir "C:\Exports" --font "C:\Fonts\MyFont.ttf"
```

Call `folio_list_fonts` with `{}` to obtain IDs, then include `fontOverrides` in
`folio_preview_paragraph`. Both modern and legacy transports use this schema.
The session retains validated font bytes at launch; changing or deleting the
original files cannot change later previews. Each worker receives private copies
of only the requested fonts, retained until it exits. Fonts are bounded separately
from the 512MiB PDF-input budget (64MiB total font bytes,16MiB per file,16 paths).
No tool accepts new font paths or scans installed fonts. Closing PDF documents
does not remove the session font catalog; the catalog ends with the session.

## Explicit fonts for single text runs

`preview-text` accepts the same repeated `--font FONT` inputs as paragraph previews.
Get the content-based ID with `fonts --font FONT`, then add an optional `fontId`
to the text request:

```json
{"page":1,"selectionId":"text-v1-<inspected hash>","replacement":"Updated text","fontId":"<font SHA256>"}
```

```powershell
.\folio-cli.exe preview-text "Report.pdf" --request "edit.json" --out-dir "C:\Exports" --font "C:\Fonts\MyFont.ttf"
```

Both page selections and `nested-text-v1-...` selections can request an explicit
font. Omit `fontId` to retain the source font. Null, invalid IDs, unauthorized
fonts, and font paths inside a request are rejected. Merely supplying `--font`
does not select it. An explicit choice is a formatting edit even when the text
is unchanged; ordinary unchanged-text previews still retain the original bytes.

MCP's `folio_preview_text` accepts the same optional `fontId`, obtained from
`folio_list_fonts`. The file must have been supplied at server launch. The
existing immutable font snapshots and file/count/byte limits apply. Neither
transport scans installed fonts or silently chooses a fallback.

Preview responses and manifests record `substituteFont` with the chosen ID, name,
weight, italic flag, and byte count. Saved previews include their font program;
Save and later source-font edits do not require the original font file. Older
requests and preview bundles without this metadata remain supported.

Single-run editing preserves the source placement and text state. Supported
positioned text, clipping, and tagged content retain native preservation checks.
Font selection does not wrap text or permit overlap: use paragraph editing when
the added words need reflow. Native character coverage, clipping, surrounding
content, and document permission checks still apply.

Nested paragraph edits promote the edited occurrence into ordinary page content.
After Apply or saving/reopening, inspect both page and nested routes again and
use the new IDs. Do not assume the edited paragraph will still be nested.
Old IDs become stale when source bytes change. Identical source bytes yield
identical selectors, but MCP document authorization is checked independently.

Nested single-run previews use manifest version 4; nested paragraph previews use
version 5, including `paragraph.occurrenceScope`. Versions 1–3 keep their previous
meanings. Page paragraphs require page seeds and paragraph-v1 IDs; nested
paragraphs require nested seeds and paragraph-v2 IDs. Mixed versions/selector
kinds and invalid or null scope metadata are refused. Both new manifests retain
the 64 KiB metadata limit and existing exact-byte Save/Apply checks. The shared
8-preview, 512 MiB and 15-minute limits and cleanup behavior are unchanged.

## Filling existing forms



Inspect a page to get its fields and their stable `fieldId` values:



```powershell

.\folio-cli.exe inspect-forms "C:\Documents\Form.pdf" --page 1

.\folio-cli.exe preview-form "C:\Documents\Form.pdf" --request "C:\Edits\answer.json" --out-dir "C:\Previews"

```



Frame-preserving answer updates also support plain dropdowns and unshaded list fields produced by common PDF libraries. Custom selection shading, scrolled list appearances, and ambiguous artwork remain guarded; an unsupported appearance is refused before publishing an edited PDF.

For example, `answer.json` contains:



```json

{"page":1,"fieldId":"field-v1-<hash from inspect-forms>","value":{"kind":"text","text":"Updated answer"}}

```



Value types are `{"kind":"text","text":"..."}`, `{"kind":"toggle","checked":true}`,

and `{"kind":"choice","indices":[0,2]}`. Choice indices are ascending, unique,

zero-based positions in the inspected options. Text fields and editable combo

boxes accept text; regular choices accept indices; checkboxes/radios accept toggles.

Empty text clears an answer. Radios must be selected by checking the desired option;

clearing a combo or selecting multiple options in a single-select field is refused.

Read-only, hidden, unsupported and incompatible fields retain native restrictions.



Inspect returns up to 25 fields and at most 512 KiB of inner JSON per batch; use

`nextOffset` until null. An individually oversized field is refused without

truncating its options or answer. The full CLI/MCP inspection response has a 2 MiB

limit including escaping, envelopes and newline. Field IDs change when source bytes,

page or native field data change. Reinspect an applied/saved version before editing.

Request files are at most 256 KiB, normalized text at most 32768 UTF-8 bytes, and

choice lists at most 2000 indices. CRLF and CR become LF. Native field length,

character and appearance-preservation constraints can impose additional limits.



Listing requires either PDF form-filling or copying permission; filling requires

form-filling permission, independently of content editing. CLI encrypted inputs

use `--password-stdin`. MCP uses configured [launch-only credentials](#multi-document-composition-in-development);
interactive or tool-supplied password entry remains unsupported. Inspection returns

field values as document data, including sensitive answers; it is not a redacted

text export. Treat PDF names, labels, options and answers as untrusted content.



The result includes `fieldId`, `form.oldValue`, `form.value`, before/after images

of the selected page, and the existing preview bundle fields. Related widgets and

radio siblings can change together across pages; the saved PDF retains those native

updates. Use `save-text-preview` for exact CLI saving (the historical command name

also accepts paragraph and form bundles), or MCP Save/Apply/Discard as above.

Unchanged text and paragraph previews preserve the exact input PDF bytes when
saved or applied, including original encryption. Existing permission and native
validation checks still apply. Text must match the inspected source text;
paragraph character/style pairs must match, with no fitting or layout change.
Splitting an equal-style run or adding an empty run does not count as an edit.
Exact echoed bounds and known equal alignment/spacing also preserve identity.
Selections remain valid for these unchanged copies; reinspect after real edits.

Preview workers retain a bounded private input snapshot so later changes at the
original path cannot change the inspected input. Windows deletes this snapshot
when the worker's handle closes, including process termination. Forced-exit
cleanup on Unix has not been verified.

Form manifests use version 3 and a 512 KiB metadata limit. Older text/paragraph

manifests keep their 64 KiB limits. All files remain inside the shared bundle and

session budgets. No source PDF is overwritten or field flattened.



### Development component: painted and scrolled Lists

The current native implementation admits selected-row backgrounds only
when source labels, option indices, scroll state, font bindings, complete paths,
paint order and measured glyph bounds prove ownership. Linked widgets keep
separate appearance/resource proof. Mixed or ambiguous artwork remains refused;
a lone visible row without observed baseline pitch cannot authorize repair.
Existing supported plain/legacy filling remains unchanged.

The commands and schemas above are unchanged. CLI Preview/Save, MCP preview
Discard and the desktop Apply/Reset/Discard draft controls were tested separately,
including saved re-editing and original-byte preservation. Native scroll state is
retained; an offscreen answer can change without changing visible pixels.
See the [reviewed component and limits](../artifacts/list-selection-proof-20261111/historical-completion.md).
The historical 0.41 portable contained only the earlier plain-List support.
Painted/scrolled-List integration is included in selected 0.50 and the separate
reviewed 0.52 candidate; see the [current build status](development-build-status.md).

## Document Find/Replace in development source

New source adds `plan-replacement` and `preview-replacement` CLI commands and
`folio_plan_replacement` / `folio_preview_replacement` MCP tools. These postdate
the preserved selected 0.50 and completed 0.52 packages; no package or launcher
promotion is implied. See [replacement planning and automation](page-replacement-planning.md)
for complete request, identity, selection and capacity details.

The planner searches the complete immutable document using the same literal
Unicode Find options, retains blocked results and returns a stable `planId`.
Preview requires that ID and the same intent, recomputes against the current
input and uses the existing complete-source batch preview. Save/Apply/Discard
operate on the entire candidate. After Apply, obtain a fresh plan for the new
document ID. This removes the need to assemble individual inspected selections.

Planning requires copy and edit permission, including no-op intents. It changes
no PDF. A selected blocker or exceeded limit cannot produce a partial Replace
all batch. Empty whole-object results and inserted line breaks still require
removal/paragraph integration. Preview retains native font, fit and preservation
checks. Desktop current-workspace Replace and Undo are later integration work.
