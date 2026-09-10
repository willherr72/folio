# Folio desktop PDF editor
Authorization: user requested autonomous overnight implementation of the discussed desktop PDF editor in a new local repository. Implementation choices and review are delegated; do not pause for further design approval.

## Outcome
A Windows desktop prototype for everyday PDF work, using Tauri 2, Rust, and native PDFium. No AI, network services, accounts, telemetry, or uploaded documents. App name Folio is a working name, not a trademark clearance.

## Scope
Open a PDF through a native dialog; view pages with a thumbnail navigator and zoom; add movable text; draw or place ink signatures; reorder, rotate, delete, duplicate and merge pages; undo/redo; export a new PDF. Source files are never silently overwritten. Native certificate signatures, OCR, existing-text replacement, forms and redaction are outside this first version.
Single selected page rendering is acceptable for this prototype; do not claim Foxit parity. Keep PDF work off the UI thread and cache rendered previews.

## Architecture
Frontend TypeScript UI owns an immutable edit plan/history and renders native PDFium PNGs plus editable overlays. Rust owns open source documents, native dialogs, rendering, validation and export. A dedicated worker serializes PDFium operations and keeps document lifetimes within its thread.
Use a bundled pinned Windows x64 PDFium binary with provenance/checksum and license files. Package a runnable desktop executable if tooling permits. Keep frontend usable with an explicit demo adapter for automated UI checks; never substitute demo behavior in the desktop app.

## Shared API (camelCase JSON)
- invoke<DocumentInfo|null>('open_pdf'): native open dialog; cancel returns null.
- invoke<ArrayBuffer>('render_page', {sourceId, pageIndex, width}): PNG bytes, width clamped to 64..2400. Original page orientation, no user rotation/overlays.
- invoke<string|null>('export_pdf', {request:{pages: PagePlan[]}}): native Save As, export and return chosen path; cancel null. Export rejects source paths.
- invoke<void>('close_document', {sourceId}): release when no longer referenced.
- invoke<string>('engine_status'): describe available engine/build.
DocumentInfo = {id:string,name:string,pages:{width:number,height:number}[]}.
PagePlan = {id:string,sourceId:string,pageIndex:number,width:number,height:number,rotation:0|90|180|270,overlays:Overlay[]}.
Overlay = TextOverlay | InkOverlay.
TextOverlay = {type:'text',id:string,x:number,y:number,text:string,fontSize:number,color:string}.
InkOverlay = {type:'ink',id:string,paths:{x:number,y:number}[][],color:string,strokeWidth:number}.
Colors #RRGGBB. Coordinates in PDF points relative to the ORIGINAL DISPLAYED page's top left, respecting its intrinsic rotation and crop. Text x/y indicates top-left of its em box; text supports multiline. Ink paths use absolute page coordinates. rotation is an ADDITIONAL clockwise rotation. Frontend transforms complete page and overlays for rotation and inversely maps pointer coordinates. Native export maps original displayed coordinates into source PDF coordinates then adds requested rotation. Exported ink/text become page content.

## Verification
Backend integration tests create sample PDFs, render, apply edits, reorder/rotate/merge, save and reopen, asserting page count/order/dimensions/text and signature pixels. Include intrinsically rotated pages and nonzero crop origin. Reject malformed payloads. UI state tests cover history, pointer transforms and page identity. UI smoke test opens demo, edits, rotates/reorders and uses export adapter. Compile/check frontend and native build; report actual results.
