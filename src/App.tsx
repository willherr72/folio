import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowDown, ArrowUp, ChevronLeft, ChevronRight, Copy, Download, FilePlus2, FolderOpen,
  GripVertical, Minus, MousePointer2, PenLine, Plus, Redo2, RotateCw, Trash2, Type, Undo2, X,
} from "lucide-react";
import { createDemoAdapter, documentToPages, nativeAdapter, type FolioAdapter } from "./editor/adapter";
import { displayDimensions } from "./editor/geometry";
import {
  commit, createHistory, deletePage, duplicatePage, movePage, planDigest, redo, removeOverlay,
  rotatePage, undo, uniqueId, updateOverlay, type EditorDocument, type History, type Overlay,
} from "./editor/model";
import type { InkPoint, PagePlan } from "./editor/types";
import { clearRenderCache, PageView, Thumbnail } from "./components/PageView";
import { SignaturePad } from "./components/SignaturePad";
import "./styles.css";

interface AppProps { initialDemo?: boolean }
type Tool = "select" | "text" | "signature";

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}



export function App({ initialDemo = new URLSearchParams(location.search).get("demo") === "1" }: AppProps) {
  const [history, setHistory] = useState<History<EditorDocument> | null>(null);
  const [savedDigest, setSavedDigest] = useState("");
  const [adapter, setAdapter] = useState<FolioAdapter>(nativeAdapter);
  const [tool, setTool] = useState<Tool>("select");
  const [zoom, setZoom] = useState(90);
  const [pendingSignature, setPendingSignature] = useState<InkPoint[][] | null>(null);
  const [signatureOpen, setSignatureOpen] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [engine, setEngine] = useState("Checking local PDF engine…");
  const [draggedPageId, setDraggedPageId] = useState<string | null>(null);
  const dragOverlay = useRef<{ document: EditorDocument; x: number; y: number; id: string } | null>(null);
  const openedSources = useRef(new Set<string>());
  const current = history?.present ?? null;
  const dirty = current ? planDigest(current) !== savedDigest : false;
  const selectedPage = current?.pages.find((page) => page.id === current.selectedPageId) ?? current?.pages[0] ?? null;
  const selectedOverlay = selectedPage?.overlays.find((overlay) => overlay.id === current?.selectedOverlayId) ?? null;

  const closeSources = useCallback(async (using: FolioAdapter) => {
    const ids = [...openedSources.current];
    openedSources.current.clear();
    clearRenderCache(ids);
    await Promise.allSettled(ids.map((sourceId) => using.closeDocument(sourceId)));
  }, []);

  const installDocument = useCallback((info: Awaited<ReturnType<FolioAdapter["openPdf"]>>, nextAdapter: FolioAdapter, append = false) => {
    if (!info) return;
    openedSources.current.add(info.id);
    const newPages = documentToPages(info);
    setAdapter(nextAdapter);
    setHistory((existing) => {
      const document: EditorDocument = append && existing
        ? { ...existing.present, name: existing.present.name, pages: [...existing.present.pages, ...newPages], selectedPageId: newPages[0]?.id ?? existing.present.selectedPageId, selectedOverlayId: null }
        : { name: info.name, pages: newPages, selectedPageId: newPages[0]?.id ?? null, selectedOverlayId: null };
      return append && existing ? commit(existing, () => document) : createHistory(document);
    });
    if (!append) setSavedDigest(planDigest({ name: info.name, pages: newPages, selectedPageId: newPages[0]?.id ?? null, selectedOverlayId: null }));
    setTool("select");
    setPendingSignature(null);
    setFailure(null);
    setNotice(append ? `${info.pages.length} pages added` : null);
    nextAdapter.engineStatus().then(setEngine).catch((error) => setEngine(`Engine unavailable · ${errorMessage(error)}`));
  }, []);

  const openDemo = useCallback(async () => {
    if (busy) return;
    const demo = createDemoAdapter();
    if (dirty && !confirm("Discard your unsaved changes and open the demo?")) return;
    setBusy("Opening demo…");
    try {
      await closeSources(adapter);
      installDocument(await demo.openPdf(), demo);
    } finally { setBusy(null); }
  }, [adapter, busy, closeSources, current, dirty, installDocument]);

  useEffect(() => { if (initialDemo && !history) void openDemo(); }, []); // explicit query flag is a user-selected demo entry

  const openPdf = useCallback(async () => {
    if (busy) return;
    if (dirty && !confirm("Discard your unsaved changes and open another PDF?")) return;
    setBusy("Opening PDF…"); setFailure(null);
    try {
      const info = await nativeAdapter.openPdf();
      if (!info) return;
      await closeSources(adapter);
      installDocument(info, nativeAdapter);
    } catch (error) { setFailure(`Couldn’t open PDF: ${errorMessage(error)}`); }
    finally { setBusy(null); }
  }, [adapter, busy, closeSources, current, dirty, installDocument]);

  const addPdf = useCallback(async () => {
    if (busy || !current || adapter.kind !== "native") return;
    setBusy("Adding PDF…"); setFailure(null);
    try { installDocument(await nativeAdapter.openPdf(), nativeAdapter, true); }
    catch (error) { setFailure(`Couldn’t add PDF: ${errorMessage(error)}`); }
    finally { setBusy(null); }
  }, [adapter.kind, busy, current, installDocument]);

  const exportPdf = useCallback(async () => {
    if (busy || !current?.pages.length) return;
    setBusy(adapter.kind === "demo" ? "Preparing demo plan…" : "Exporting PDF…"); setFailure(null);
    try {
      const path = await adapter.exportPdf(current.pages);
      if (path) { setSavedDigest(planDigest(current)); setNotice(adapter.kind === "demo" ? "Demo edit plan downloaded" : `Saved a copy to ${path}`); }
    } catch (error) { setFailure(`Couldn’t export: ${errorMessage(error)}`); }
    finally { setBusy(null); }
  }, [adapter, busy, current]);

  const edit = useCallback((update: (document: EditorDocument) => EditorDocument) => {
    setHistory((value) => value ? commit(value, update) : value);
  }, []);

  const selectPage = (pageId: string) => setHistory((value) => value ? { ...value, present: { ...value.present, selectedPageId: pageId, selectedOverlayId: null } } : value);
  const selectOverlay = (id: string | null) => setHistory((value) => value ? { ...value, present: { ...value.present, selectedOverlayId: id } } : value);

  const addText = (point: InkPoint) => {
    if (!selectedPage) return;
    const id = uniqueId("text");
    edit((document) => ({
      ...document,
      pages: document.pages.map((page) => page.id === selectedPage.id ? { ...page, overlays: [...page.overlays, { type: "text", id, x: point.x, y: point.y, text: "Type here", fontSize: 18, color: "#2D2A26" }] } : page),
      selectedOverlayId: id,
    }));
    setTool("select");
  };

  const placeSignature = (point: InkPoint) => {
    if (!selectedPage || !pendingSignature) return;
    const points = pendingSignature.flat();
    const minX = Math.min(...points.map((value) => value.x)), minY = Math.min(...points.map((value) => value.y));
    const maxX = Math.max(...points.map((value) => value.x)), maxY = Math.max(...points.map((value) => value.y));
    const scale = Math.min(0.55, 180 / Math.max(1, maxX - minX), 72 / Math.max(1, maxY - minY));
    const paths = pendingSignature.map((path) => path.map((value) => ({ x: Math.min(selectedPage.width, point.x + (value.x - minX) * scale), y: Math.min(selectedPage.height, point.y + (value.y - minY) * scale) })));
    const id = uniqueId("signature");
    edit((document) => ({ ...document, pages: document.pages.map((page) => page.id === selectedPage.id ? { ...page, overlays: [...page.overlays, { type: "ink", id, paths, color: "#2D2A26", strokeWidth: 2 }] } : page), selectedOverlayId: id }));
    setPendingSignature(null); setTool("select"); setNotice("Signature placed");
  };

  const moveOverlay = (id: string, x: number, y: number, phase: "start" | "move" | "end") => {
    if (!selectedPage || !history) return;
    if (phase === "start") { dragOverlay.current = { document: history.present, x, y, id }; return; }
    const start = dragOverlay.current;
    if (!start || start.id !== id) return;
    if (phase === "end") {
      setHistory((value) => value && planDigest(value.present) !== planDigest(start.document)
        ? { past: [...value.past, start.document], present: value.present, future: [] } : value);
      dragOverlay.current = null;
      return;
    }
    const dx = x - start.x, dy = y - start.y;
    const sourcePage = start.document.pages.find((page) => page.id === selectedPage.id);
    const sourceOverlay = sourcePage?.overlays.find((overlay) => overlay.id === id);
    if (!sourceOverlay) return;
    setHistory((value) => value ? { ...value, present: updateOverlay(value.present, selectedPage.id, id, () => sourceOverlay.type === "text"
      ? { ...sourceOverlay, x: sourceOverlay.x + dx, y: sourceOverlay.y + dy }
      : { ...sourceOverlay, paths: sourceOverlay.paths.map((path) => path.map((value) => ({ x: value.x + dx, y: value.y + dy }))) }) } : value);
  };

  const chooseSignature = () => { setSignatureOpen(true); setTool("signature"); };
  const acceptSignature = (paths: InkPoint[][]) => { setSignatureOpen(false); setPendingSignature(paths); setNotice("Click the page to place your signature"); };

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const command = event.ctrlKey || event.metaKey;
      if (command && event.key.toLowerCase() === "o") { event.preventDefault(); void openPdf(); }
      if (command && event.key.toLowerCase() === "s") { event.preventDefault(); void exportPdf(); }
      if (command && event.key.toLowerCase() === "z" && !event.shiftKey) { event.preventDefault(); setHistory((value) => value ? undo(value) : value); }
      if (command && (event.key.toLowerCase() === "y" || (event.shiftKey && event.key.toLowerCase() === "z"))) { event.preventDefault(); setHistory((value) => value ? redo(value) : value); }
      if ((event.key === "Delete" || event.key === "Backspace") && selectedPage && selectedOverlay && !["INPUT", "TEXTAREA"].includes((event.target as HTMLElement).tagName)) edit((document) => removeOverlay(document, selectedPage.id, selectedOverlay.id));
      if (event.key === "Escape") { setTool("select"); setPendingSignature(null); selectOverlay(null); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [edit, exportPdf, openPdf, selectedOverlay, selectedPage]);

  useEffect(() => {
    const beforeUnload = (event: BeforeUnloadEvent) => { if (dirty) { event.preventDefault(); event.returnValue = ""; } };
    window.addEventListener("beforeunload", beforeUnload);
    let unlisten: undefined | (() => void);
    let cancelled = false;
    if (adapter.kind === "native") {
      import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().onCloseRequested(async (event) => {
        if (dirty && !confirm("Close Folio and discard unsaved changes?")) event.preventDefault();
      })).then((dispose) => { if (cancelled) dispose(); else unlisten = dispose; }).catch(() => {});
    }
    return () => { cancelled = true; window.removeEventListener("beforeunload", beforeUnload); unlisten?.(); };
  }, [adapter.kind, dirty]);

  const selectedIndex = selectedPage ? current?.pages.findIndex((page) => page.id === selectedPage.id) ?? -1 : -1;
  const displaySize = selectedPage ? displayDimensions(selectedPage.width, selectedPage.height, selectedPage.rotation) : null;

  if (!current) return <main className="empty-shell">
    <div className="empty-brand"><span className="folio-mark">F</span><span>Folio</span></div>
    <section className="empty-card">
      <div className="empty-illustration" aria-hidden="true"><div className="sheet sheet-back"/><div className="sheet sheet-front"><span/><span/><i/></div></div>
      <p className="eyebrow">A PRIVATE PDF WORKSPACE</p>
      <h1>Make PDFs feel finished.</h1>
      <p className="empty-copy">Arrange pages, add text, and place your signature in a focused desktop workspace. Your documents stay on this device.</p>
      <div className="empty-actions"><button className="button primary large" onClick={openPdf}><FolderOpen size={18}/> Open a PDF</button><button className="button large" onClick={openDemo}>Explore demo</button></div>
      {busy && <p className="busy-note">{busy}</p>}{failure && <p className="error-note">{failure}</p>}
      <p className="privacy-note"><span>●</span> Local editing · Source files remain untouched</p>
    </section>
  </main>;

  return <div className="app-shell">
    <header className="titlebar">
      <div className="brand"><span className="folio-mark small">F</span><span>Folio</span></div>
      <div className="document-title"><span>{current.name}</span>{dirty && <i aria-label="Unsaved changes"/>}{adapter.kind === "demo" && <b>DEMO</b>}</div>
      <div className="engine-pill"><span className={engine.startsWith("Engine unavailable") ? "offline" : ""}/>{engine}</div>
    </header>

    <div className="toolbar" role="toolbar" aria-label="Document tools">
      <div className="tool-group"><button className="tool-button" onClick={openPdf} disabled={!!busy} title="Open PDF (Ctrl+O)"><FolderOpen size={17}/><span>Open</span></button><button className="tool-button" onClick={addPdf} disabled={!!busy || adapter.kind === "demo"} title="Add another PDF"><FilePlus2 size={17}/><span>Add PDF</span></button></div>
      <div className="separator"/>
      <div className="tool-group modes"><button className={`tool-button ${tool === "select" ? "active" : ""}`} onClick={() => { setTool("select"); setPendingSignature(null); }}><MousePointer2 size={17}/><span>Select</span></button><button className={`tool-button ${tool === "text" ? "active" : ""}`} onClick={() => { setTool("text"); setPendingSignature(null); }}><Type size={17}/><span>Text</span></button><button className={`tool-button ${tool === "signature" ? "active" : ""}`} onClick={chooseSignature}><PenLine size={17}/><span>Signature</span></button></div>
      <div className="separator"/>
      <div className="tool-group"><button className="icon-button" aria-label="Undo" disabled={!history?.past.length} onClick={() => setHistory((value) => value ? undo(value) : value)}><Undo2 size={18}/></button><button className="icon-button" aria-label="Redo" disabled={!history?.future.length} onClick={() => setHistory((value) => value ? redo(value) : value)}><Redo2 size={18}/></button></div>
      <div className="toolbar-spacer"/>
      <div className="zoom-control"><button aria-label="Zoom out" onClick={() => setZoom((value) => Math.max(50, value - 10))}><Minus size={15}/></button><button className="zoom-value" onClick={() => setZoom(100)}>{zoom}%</button><button aria-label="Zoom in" onClick={() => setZoom((value) => Math.min(200, value + 10))}><Plus size={15}/></button></div>
      <button className="button primary export" onClick={exportPdf} disabled={!!busy}><Download size={17}/>{adapter.kind === "demo" ? "Export demo plan" : "Save a copy"}</button>
    </div>

    <div className="workspace">
      <aside className="sidebar">
        <div className="panel-heading"><span>PAGES</span><em>{current.pages.length}</em></div>
        <div className="thumbnails">
          {current.pages.map((page, index) => <button key={page.id} className={`thumbnail-item ${page.id === selectedPage?.id ? "selected" : ""}`} aria-label={`Page ${index + 1} of ${current.pages.length}`} draggable
            onDragStart={() => setDraggedPageId(page.id)} onDragOver={(event) => event.preventDefault()} onDrop={() => { if (draggedPageId) edit((document) => movePage(document, draggedPageId, index)); setDraggedPageId(null); }} onClick={() => selectPage(page.id)}>
            <span className="drag-handle"><GripVertical size={14}/></span><div className="thumbnail-paper"><Thumbnail adapter={adapter} page={page}/></div><span className="page-number">{index + 1}</span>
          </button>)}
        </div>
        <button className="add-pages" onClick={addPdf} disabled={!!busy || adapter.kind === "demo"}><Plus size={16}/> Add pages</button>
      </aside>

      <main className="viewport">
        {pendingSignature && <div className="placement-banner"><PenLine size={16}/> Click anywhere on the page to place your signature <button aria-label="Cancel signature placement" onClick={() => { setPendingSignature(null); setTool("select"); }}><X size={15}/></button></div>}
        {selectedPage && <PageView adapter={adapter} page={selectedPage} pageNumber={selectedIndex + 1} zoom={zoom} tool={tool} selectedOverlayId={current.selectedOverlayId} pendingSignature={pendingSignature} onSelectOverlay={selectOverlay} onAddText={addText} onPlaceSignature={placeSignature} onMoveOverlay={moveOverlay}/>}
        <div className="page-nav"><button aria-label="Previous page" disabled={selectedIndex <= 0} onClick={() => selectPage(current.pages[selectedIndex - 1].id)}><ChevronLeft size={16}/></button><span>Page {selectedIndex + 1} of {current.pages.length}</span><button aria-label="Next page" disabled={selectedIndex >= current.pages.length - 1} onClick={() => selectPage(current.pages[selectedIndex + 1].id)}><ChevronRight size={16}/></button></div>
      </main>

      <aside className="properties">
        <div className="panel-heading"><span>PROPERTIES</span></div>
        {selectedOverlay && selectedPage ? <OverlayProperties overlay={selectedOverlay} onChange={(update) => edit((document) => updateOverlay(document, selectedPage.id, selectedOverlay.id, update))} onDelete={() => edit((document) => removeOverlay(document, selectedPage.id, selectedOverlay.id))}/> : selectedPage && <>
          <section className="property-section"><h3>Page</h3><div className="page-summary"><div className="mini-page" style={{ aspectRatio: `${displaySize!.width}/${displaySize!.height}` }}/><div><strong>Page {selectedIndex + 1}</strong><span>{Math.round(selectedPage.width)} × {Math.round(selectedPage.height)} pt</span><small>{selectedPage.rotation ? `${selectedPage.rotation}° clockwise` : "Original orientation"}</small></div></div></section>
          <section className="property-section"><h3>Arrange</h3><div className="property-grid"><button disabled={selectedIndex <= 0} onClick={() => edit((document) => movePage(document, selectedPage.id, selectedIndex - 1))}><ArrowUp size={16}/>Move up</button><button disabled={selectedIndex >= current.pages.length - 1} onClick={() => edit((document) => movePage(document, selectedPage.id, selectedIndex + 1))}><ArrowDown size={16}/>Move down</button><button onClick={() => edit((document) => rotatePage(document, selectedPage.id))}><RotateCw size={16}/>Rotate</button><button onClick={() => edit((document) => duplicatePage(document, selectedPage.id, uniqueId("page")))}><Copy size={16}/>Duplicate</button></div></section>
          <section className="property-section"><h3>Page actions</h3><button className="danger-action" disabled={current.pages.length <= 1} onClick={() => edit((document) => deletePage(document, selectedPage.id))}><Trash2 size={16}/>Delete page</button><p>Source files are never changed.</p></section>
        </>}
      </aside>
    </div>

    <footer className="statusbar"><span>{busy ?? notice ?? (dirty ? "Unsaved edits" : "All changes exported")}</span>{failure && <span className="status-error">{failure}</span>}<span className="status-spacer"/><span>{selectedPage ? `${Math.round(selectedPage.width)} × ${Math.round(selectedPage.height)} pt` : ""}</span><span>Local only</span></footer>
    {signatureOpen && <SignaturePad onCancel={() => { setSignatureOpen(false); setTool("select"); }} onAccept={acceptSignature}/>}
  </div>;
}

function OverlayProperties({ overlay, onChange, onDelete }: { overlay: Overlay; onChange(update: (value: Overlay) => Overlay): void; onDelete(): void }) {
  return <>
    <section className="property-section"><h3>{overlay.type === "text" ? "Text" : "Signature"}</h3>{overlay.type === "text" ? <>
      <label className="field-label">Content<textarea value={overlay.text} rows={4} onChange={(event) => onChange((value) => value.type === "text" ? { ...value, text: event.target.value } : value)}/></label>
      <div className="field-row"><label className="field-label">Size<input type="number" min="6" max="96" value={overlay.fontSize} onChange={(event) => onChange((value) => value.type === "text" ? { ...value, fontSize: Math.max(6, Math.min(96, Number(event.target.value))) } : value)}/></label><label className="field-label">Color<span className="color-input"><input type="color" value={overlay.color} onChange={(event) => onChange((value) => ({ ...value, color: event.target.value.toUpperCase() }))}/><code>{overlay.color}</code></span></label></div>
      <p>Helvetica · {overlay.text.split("\n").length} {overlay.text.includes("\n") ? "lines" : "line"}</p>
    </> : <><label className="field-label">Ink color<span className="color-input"><input type="color" value={overlay.color} onChange={(event) => onChange((value) => ({ ...value, color: event.target.value.toUpperCase() }))}/><code>{overlay.color}</code></span></label><label className="field-label">Stroke width<input type="range" min="0.5" max="6" step="0.5" value={overlay.strokeWidth} onChange={(event) => onChange((value) => value.type === "ink" ? { ...value, strokeWidth: Number(event.target.value) } : value)}/></label></>}</section>
    <section className="property-section"><h3>Position</h3><p>Drag this item directly on the page to move it.</p><button className="danger-action" onClick={onDelete}><Trash2 size={16}/>Delete {overlay.type === "text" ? "text" : "signature"}</button></section>
  </>;
}