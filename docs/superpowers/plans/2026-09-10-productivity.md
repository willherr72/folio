# Folio Productivity Implementation Plan
> For agentic workers: use superpowers:subagent-driven-development and requesting-code-review.
Goal: ship immediate text editing, signature placement preview, native text selection and document tabs.
Architecture: PDFium character geometry feeds a browser selection layer; active React session selects independent per-tab histories.
Tech stack: React, TypeScript, Tauri, Rust/PDFium.
Spec: docs/superpowers/specs/2026-09-10-productivity-design.md

## Global Constraints
All PDF processing remains local. Preserve existing source safety and vector export. No OCR or existing-PDF text editing. Keep independent source lifetime and histories per tab. The existing authorization covers routine design, isolated work, source pushes and releases.

## Task 1 — Native text geometry (delegated)
Files: src-tauri/src/{engine,types,app,lib}.rs; src-tauri/tests/text_geometry.rs; src/editor/{types,adapter}.ts.
- [ ] Write failing tests for page_text extraction, text/space preservation, crop and intrinsic rotation, invalid source/page.
- [ ] Add PdfTextCharacter and PageText with the exact interface from the spec; add optional getPageText to FolioAdapter and native/demo implementations.
- [ ] Add a serialized worker request and public engine method page_text. Use existing page normalization to return top-left rendered coordinates; preserve all text including whitespace.
- [ ] Expose page_text through Tauri; run focused native tests and fmt; commit owned files and report.
Contract: getPageText?(sourceId:string,pageIndex:number):Promise<PageText>; PageText.characters has {text,x,y,width,height} in unrotated PagePlan coordinates.

## Task 2 — Signature preview and selection layer (delegated after Task 1)
Files: src/components/PageView.tsx, new PdfTextLayer.tsx and page-interactions.css; new focused selection/preview tests.
- [ ] Add failing tests for signature ghost alignment/cancel and actual PDF text selection/copy.
- [ ] Request native character geometry only for mounted page views; gracefully retain viewing when text unavailable.
- [ ] Render an accessible transparent text layer with native selection in Select mode; preserve spaces/newlines when copying, rotate/scale consistently, keep editable overlays above it.
- [ ] Add a pointer-following pending signature using placeInkPaths; hide on leave/cancel/disabled. Add a visible placement cue.
- [ ] Clear text geometry cache with existing clearRenderCache source cleanup; run focused checks and commit owned files.
Do not edit App, global styles.css, DocumentViewport, adapter/types, or native files. Consume Task 1 interface.

## Task 3 — Tabs and immediate typing (parent)
Files: src/App.tsx, src/editor/workspace.ts, src/components/DocumentViewport.tsx, styles.css; tests/workspace.test.ts and app productivity tests.
- [ ] Write failure tests for independent tab history/dirty state and immediate placeholder replacement.
- [ ] Add DocumentSession state with id, history, savedDigest, adapter, sourceIds, zoom, navigationRequest and scroll position. Derive active history/setters by ID.
- [ ] Change Open to add tabs; append remains active-tab only; preserve cancelled picker state. Render top tab strip with accessible labels, close controls, unsaved markers and new-tab affordance.
- [ ] Close dirty tabs via modal and release only their source IDs. Aggregate all dirty tabs for native close. Block tab changes during modal/native operations. Add Ctrl+Tab and Ctrl+W.
- [ ] Focus/select Content on newly created text IDs. Save/reinstate viewport scroll per active tab without triggering unsolicited navigation.
- [ ] Run frontend checks and browser interaction; commit integration.

## Task 4 — Review and delivery (parent)
- [ ] Obtain independent spec/quality review of all tasks, resolve meaningful findings.
- [ ] Run full frontend/native checks; adapt browser/native smoke to tabs and text selection.
- [ ] Build isolated versioned portable update; verify saved PDF and archive executable identity.
- [ ] Merge reviewed branch, update launcher/docs/changelog, push and publish versioned release with checksum.
