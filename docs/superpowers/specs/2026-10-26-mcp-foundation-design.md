# Folio local MCP foundation

User intent: continue Folio, learn useful automation from PDFCraft, retain the
existing PDF engine and preservation checks. User delegates routine choices and
approval questions. The decisions below are implementation choices, not a claim
that the user prescribed this API.

## Approach

Implement `folio-cli mcp --input PDF [--input PDF ...] --out-dir DIRECTORY
[--engine DLL]` in Rust, without new dependencies. A small stdio transport starts
the same executable's existing CLI commands as private one-operation workers.
This keeps exactly the shared desktop operations and isolates native lifetime,
crashes and cancellation. Direct persistent engine calls would be faster but
require a broader font/resource retention audit. A separate JS service would add
a second deployment/runtime. Neither is necessary for this milestone.

## Contract

- Local stdio only, no listener, shell, remote authentication, GUI attachment,
  editing, or passwords. Protected documents report existing engine errors.
- Launch-time file authorization: 1–64 explicit regular input files, copied to
  private session snapshots with an aggregate 512 MiB streaming limit. Tools
  address `document-1` etc., never paths. Original files remain unchanged; later
  original changes require restart. Native filenames are display labels, not IDs.
- Output parent is explicit at launch. Each export uses its own generated child
  directory. No client-supplied paths or filenames. Success persists exports;
  failure/cancellation removes only this operation's private temporary directory.
  This is a boundary against tool arguments, not hostile same-user filesystem
  mutation or an OS sandbox. Existing native limits/permissions remain enforced.
- Tools: `folio_list_documents` (IDs/names/bytes), `folio_info` (metadata),
  `folio_export_text` (optional 1-based page, opt-in form values; path + first
  65536 UTF-8 bytes with truncation flag), `folio_render_page` (required 1-based
  page, DPI 72/150/300/600 default150; generated PNG folder). Complete text stays
  in the exported UTF-8 file. Text must be labeled document content, not commands.
- Current MCP 2026-07-28: server/discover, per-request protocolVersion and
  clientCapabilities metadata, optional validated clientInfo, complete results.
  Legacy initialization supports 2025-11-25, 2025-06-18 and 2024-11-05. Modern
  metadata always determines its own request semantics. Never silently interpret
  unsupported modern versions as legacy. Unknown legacy proposals negotiate
  2025-11-25. Metadata-less tools require completed legacy initialization.
- JSON-RPC newline framing; stdout contains protocol only. Strict tool schemas,
  notifications have no replies, protocol errors differ from tool operation
  failures. Input line max64KiB, bounded32-message channel, response max16MiB.
  One native operation at a time; another native call gets an explicit busy
  error. Discovery/list/ping remain responsive. Calls have a 120-second deadline.
  Cancellation and EOF kill and reap the owned worker; cancelled calls emit no
  later response. No task/progress/resource capabilities advertised.
- After final-review reproduction: stdout uses a separate writer, at most eight
  queued replies, each below 16 MiB including its newline. Queue full/broken means
  session shutdown and owned-worker cleanup; supervisor never blocks sending or
  joining the writer. Legacy ping is permitted throughout initialization, but
  explicit modern metadata is always validated before any legacy state mutation.
  Windows renderer paths use OS-aware absolute resolution at launch, including
  drive-relative names, without requiring a DLL just for discovery.
- Snapshot paths, stderr, passwords, raw arguments and internal document IDs
  never appear in normal tool errors. Output path native encoding inherits CLI.
- Help/discovery/list need no PDFium. Renderer resolution remains explicit or
  executable-relative, independent of working directory.

## Verification

Subprocess tests exercise both protocol eras, framing recovery, bounds, strict
arguments, unapproved path rejection, snapshot stability, actual info/text/PNG,
source hashes, protected/malformed PDFs, repeated calls, cancellation/EOF and
no-clobber output. Worker termination tested deterministically in module tests.
Run CLI + MCP + native library/export suites once complete, build portable CLI,
exercise from outside checkout, and obtain one fresh-context scoped final review.
No commercial parity or third-party client compatibility claim without testing.

## Protocol sources checked 2026-10-26

- https://modelcontextprotocol.io/specification/2026-07-28/server/discover
- https://modelcontextprotocol.io/specification/2026-07-28/server/tools
- https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio
- https://modelcontextprotocol.io/specification/2026-07-28/schema
- https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle

Schema clarifies clientInfo is optional, unlike abbreviated examples. Unsupported
modern versions use -32022 with requested/supported data. Cacheable modern
discovery/tool-list replies require ttlMs and cacheScope. Use0/private explicitly;
the official2.4.0 client SDK must accept both discovery and list/tool operations.

SDK acceptance exposed that negotiated/pinned stdio clients start a disposable
discovery sibling and terminate it without waiting for application destructors.
Windows input snapshots must therefore use individual named files with kernel
delete-on-close handles retained for the session, not destructor-only directories.
Keep create-new semantics and Unix0600 permissions. Graceful lifetime tests remain;
add actual terminated-probe cleanup regression and repeat all native/SDK acceptance.
