# Folio CLI foundation

The user chose Folio-first development and specifically values CLI/MCP automation.
Build a usable Windows CLI milestone before adding a protocol server or edit sessions.
Standing authorization delegates routine design choices; no new approval gate.

Use the existing `folio_engine::PdfEngine` through a small shared automation module.
Do not drive the GUI or implement another PDF parser, renderer, or exporter. This
provides the future MCP dispatch boundary, but MCP is not part of this milestone.

## User contract

- New console binary `folio-cli`; keep `cargo run` targeting the desktop `folio`.
- `info INPUT.pdf`: JSON name, page count, physical page sizes and protection info;
  no document contents, bookmark graph, overlays or internal source IDs.
- `text INPUT.pdf --out OUTPUT.txt [--page N] [--include-form-values]`: desktop UTF-8
  text export, including its BOM/page separators, OCR and annotation behavior.
  All pages by default, or one 1-based page. Form values are opt-in.
- `render INPUT.pdf --out-dir EXISTING_DIR [--page N] [--dpi 72|150|300|600]`:
  desktop PNG export in a new uniquely owned child folder; default 150 DPI.
- JSON success on stdout (`schemaVersion:1`, `ok:true`, `command`, `result`). JSON
  failure on stderr (`schemaVersion:1`, `ok:false`, `error:{code,message}`); no
  PDF contents/passwords in diagnostics. Exit 0 success, 2 arguments, 1 operation.
- `--help` and `--version` work without loading PDFium. Reject unknown, duplicate,
  missing or command-inappropriate options. Use native path arguments (OsString).
- `--engine PATH` selects PDFium explicitly. Otherwise locate the packaged
  `resources/pdfium/pdfium.dll` next to the CLI, never search the working directory.
- Optional `--password-stdin` reads at most 1027 bytes to enforce at most 1024 UTF-8
  password bytes after removing one final LF/CRLF. Reject interior CR/LF and NUL.
  Do not accept passwords as arguments or retain/log them; use Sensitive storage.
- At most 10,000 pages per operation. Preserve source PDF permissions and existing
  desktop limits. Each operation owns and closes its native document on every exit.
- Never replace an existing text destination, including source aliases or symlinks.
  Publish with exclusive no-clobber semantics after exporting to private staging in
  the destination parent. Existing PNG export already owns an exclusive child folder.
- Paths explicitly supplied by the invoking user are authorized local paths. This
  CLI is not a sandbox or network service. Future MCP must design its own path/session
  boundary rather than blindly expose this entry point to remote callers.
- Output path JSON is explicitly lossless: ordinary `path`/`folder` Unicode
  strings remain unchanged; non-Unicode names use null plus `pathNative` or
  `folderNative` with Windows UTF-16 units or Unix bytes and an encoding label.
  Do not serialize PathBuf through json! or publish then fail to describe its name.

## Scope and alternatives

Chosen: one-shot operations over saved files, typed shared request and JSON output.
Deferred: persistent sessions, unsaved desktop state, PDF modifications, password
prompts, JSON-RPC/MCP, cancellation protocol, stdout document data and overwrite flags.
These have separate lifecycle contracts. A GUI-control wrapper would be brittle;
replacing PDFium or adopting PDFCraft's object model would duplicate current work.

## Acceptance

Run actual subprocesses for help/version/invalid arguments, Unicode input/output
paths, info, all-page/selected-page text and PNG output. Compare text byte-for-byte
and PNG decoded pixels with the desktop engine exports from the same saved PDF.
Cover native comments/forms, rotated/UserUnit pages, zero/out-of-range pages,
missing renderer, malformed input, protected PDFs, password bounds and preserved
source/output sentinels (including hard-link aliases). Failure must not leave a
published partial text file or newly registered source. Stage a runnable CLI with
the exact renderer and notices, and exercise it outside the repository directory.

No claim of MCP support, cross-platform acceptance, release readiness, or parity.
