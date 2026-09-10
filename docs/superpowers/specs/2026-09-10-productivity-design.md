# Folio productivity update
User intent: immediately type after creating text; preview a pending signature at the pointer; select/copy embedded PDF text; open multiple PDFs in top tabs. These requested changes authorize implementation and normal release delivery under the established workflow.

## Design
Tabs are an architectural change. Keep independent document sessions in React state and render only the active session. Each session retains history, saved digest, adapter/source ownership, zoom, page navigation and viewport scroll position. The alternatives are keeping every editor mounted (wastes rendering and duplicates event listeners) or opening separate windows (does not provide the requested tabs). Opening a PDF creates a tab without discarding another tab. Add PDF still appends pages to the active document. Closing a dirty tab prompts; closing the app considers every tab. Cancelling a picker or close preserves all sessions. Switching clears transient selection gestures/signature placement, but not document content. Ctrl+Tab/Ctrl+Shift+Tab switch and Ctrl+W closes the active tab.

New text selects its placeholder in the Content editor immediately, so typing replaces it without another click. Existing text selection should not repeatedly steal focus. Choosing a signature shows a translucent actual-size preview at the pointer on the page, with a placement hint and Escape cancellation; the preview uses exactly the same coordinate and clamping helper as placement.

PDFium supplies characters and their bounds in the same top-left normalized page coordinates as the rendered image (crop and intrinsic rotation included). A transparent browser text layer exposes native range selection and copying in Select mode, with edit overlays above it and other tools disabling text hit testing. Copy preserves spaces/newlines, and added text remains editable. Text geometry is loaded only for visible/nearby pages, cached with bounded lifetime, and released for closed sources. Scanned pages without embedded text remain image-only; OCR is outside this update.

## Contract
TypeScript:
interface PdfTextCharacter { text: string; x: number; y: number; width: number; height: number }
interface PageText { intrinsicRotation?: 0 | 90 | 180 | 270; characters: PdfTextCharacter[] }
FolioAdapter.getPageText?(sourceId: string, pageIndex: number): Promise<PageText>
Native command: page_text(sourceId, pageIndex) -> PageText.
Geometry is normalized before user-applied PagePlan.rotation. intrinsicRotation preserves the source glyph axes for browser caret direction; absent values mean zero. New source IDs are owned by exactly one tab, including appended sources; closing a tab releases only that ownership.

## Validation and delivery
Meaningful tests cover immediate typing, signature preview/placement/cancel, native character geometry under crop/intrinsic rotation, text copying, independent tab history and selection/zoom, picker cancellation, tab close and all-tab dirty close. Run frontend/typecheck/native checks, actual browser interaction, and packaged native PDF edit/save/reopen. Preserve the existing release and publish a verified versioned update to the established GitHub repo.
