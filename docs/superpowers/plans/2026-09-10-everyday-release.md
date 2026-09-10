# Everyday PDF Release Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans; independent native subsystems use superpowers:dispatching-parallel-agents. Steps use checkbox syntax.

**Goal:** Deliver v0.4.0 with local crash recovery, document search, and Windows printing.
**Architecture:** Independent recovery store, search engine/UI, and print backend meet at thin Tauri commands and App integration. Recovery stores source snapshots plus versioned view/edit state.
**Tech Stack:** Rust, Tauri 2, React 18, TypeScript, PDFium, Win32 GDI.
**Spec:** ../specs/2026-09-10-everyday-release-design.md

## Global Constraints
Windows x64; no cloud or AI. Original files remain untouched. Native validation prints only to Microsoft Print to PDF. Later milestones stay out of scope. Preserve bounded tab rendering caches.

### Task 1: Native recovery
Files: src-tauri/src/recovery.rs, engine.rs, tests/recovery_store.rs.
Interface: RecoveryStore::new(PathBuf), save(&PdfEngine, serde_json::Value), load(&PdfEngine)->Option<Value>, clear(). Native errors are actionable strings; lifetime store lock prevents instance races.
- [ ] Write and run failing tests for source snapshots, manifest fallback and ID remapping.
- [ ] Snapshot immutable opened source bytes on the worker; atomic versioned manifests retain last good state.
- [ ] Validate schema and snapshot names; test interrupted writes, missing originals, corrupt own sources, exclusive lock and cleanup.
- [ ] Run targeted native tests and inspect changes.

### Task 2: Search
Files: src/editor/search.ts, components/SearchBar.tsx, PdfTextLayer.tsx, PageView.tsx, DocumentViewport.tsx, search CSS and tests.
Interface: useDocumentSearch(adapter,pages,query,enabled)->{matches,searching,error,hasText}; SearchMatch={id,pageId,rects:[{x,y,width,height}]}. Viewport accepts matches, active ID, navigationRequest.rect.
- [ ] Write failing tests for normalized phrase matches, repeated/duplicated pages and stale asynchronous work.
- [ ] Implement bounded text retrieval and responsive result scanning.
- [ ] Add controlled search bar and SVG rectangles; navigate rotated match bounds.
- [ ] Run targeted unit and browser geometry checks.

### Task 3: Native printing
Files: src-tauri/src/printing.rs, Cargo.toml/Cargo.lock and tests.
Interface: print_document(PdfEngine,ExportRequest,PrintOptions)->Result<bool,String>; options scale=fit|actual, orientation=portrait|landscape. False means cancel.
- [ ] Write failing printable-area, scale, pixel-budget and cancellation tests.
- [ ] Implement native print dialog and sequential GDI spooling from an edited temporary PDF.
- [ ] Validate DPI, margins, printer failure and temporary document cleanup.
- [ ] Build Windows backend; inspect output through virtual printer during integration.

### Task 4: App integration
Files: App.tsx, editor/recovery.ts, editor/printing.ts, components/PrintDialog.tsx, RecoveryDialog.tsx, workspace.ts, src-tauri/src/app.rs/lib.rs.
Recovery JSON: {version:1,activeId,tabs:[{id,document,savedDigest,dirty,zoom,scrollPosition}]}.
- [ ] Write failing tests for restoring dirty tabs with new source IDs, serialized saves/clear, invalid ranges, and keyboard/dialog flows.
- [ ] Implement typed IPC wrappers and per-tab search state.
- [ ] Gate startup on recovery choice; checkpoint workspace/scroll changes, remove closed tabs before releasing sources, drain/clear on normal window close.
- [ ] Add print controls and range validation preserving page order/edits.
- [ ] Register native commands and store; run frontend build and all tests.

### Task 5: Release verification
Files: native/browser smoke scripts, README.md, CHANGELOG.md, docs/verification.md, docs/releases/v0.4.0.md, version metadata and launcher.
- [ ] Baseline: 68 frontend tests passed on clean worktree.
- [ ] Run native suite, frontend suite/build, browser interactions and tab benchmark.
- [ ] Build desktop; verify real crash/restart and search/navigation with edited multi-tab PDFs.
- [ ] Print to PDF and reopen/inspect page selection, rotations and edits.
- [ ] Obtain independent code review, fix findings, rerun affected checks.
- [ ] Package notices/portable executable, merge verified branch, publish release, close delivered issues and milestone.
