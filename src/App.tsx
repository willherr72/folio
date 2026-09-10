import { useCallback, useEffect, useRef, useState } from "react";
import {
  ArrowDown, ArrowUp, ChevronLeft, ChevronRight, Copy, Download, FilePlus2, FolderOpen,
  GripVertical, Minus, MousePointer2, PenLine, Pencil, Plus, Redo2, RotateCw, Settings, Trash2, Type, Undo2, X,
} from "lucide-react";
import { createDemoAdapter, documentToPages, nativeAdapter, type FolioAdapter } from "./editor/adapter";
import { displayDimensions, placeInkPaths } from "./editor/geometry";
import {
  commit, createHistory, deletePage, duplicatePage, movePage, planDigest, redo, removeOverlay,
  rotatePage, undo, uniqueId, updateOverlay, type EditorDocument, type History, type Overlay,
} from "./editor/model";
import type { InkPoint } from "./editor/types";
import { usePreferences } from "./editor/preferences";
import { clearRenderCache, Thumbnail } from "./components/PageView";
import { DocumentViewport } from "./components/DocumentViewport";
import { SignaturePad } from "./components/SignaturePad";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { SettingsDialog } from "./components/SettingsDialog";
import "./styles.css";

interface AppProps { initialDemo?: boolean }
type Tool = "select" | "text" | "signature" | "draw";
type Confirmation = { title: string; description: string; confirmLabel: string };
function errorMessage(error: unknown) { return error instanceof Error ? error.message : String(error); }

export function App({ initialDemo = new URLSearchParams(location.search).get("demo") === "1" }: AppProps) {
  const { preferences, setPreferences, resetPreferences } = usePreferences();
  const [history, setHistory] = useState<History<EditorDocument> | null>(null);
  const [savedDigest, setSavedDigest] = useState("");
  const [adapter, setAdapter] = useState<FolioAdapter>(nativeAdapter);
  const [tool, setTool] = useState<Tool>("select");
  const [zoom, setZoom] = useState(preferences.defaultZoom);
  const [pendingSignature, setPendingSignature] = useState<InkPoint[][] | null>(null);
  const [signatureOpen, setSignatureOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [draggedPageId, setDraggedPageId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<{ pageId: string; side: "before" | "after" } | null>(null);
  const [navigationRequest, setNavigationRequest] = useState<{ pageId: string; revision: number } | null>(null);
  const dragOverlay = useRef<{ document: EditorDocument; pageId: string; x: number; y: number; id: string } | null>(null);
  const openedSources = useRef(new Set<string>());
  const operation = useRef(false);
  const decision = useRef<((accepted: boolean) => void) | null>(null);
  const current = history?.present ?? null;
  const dirty = current ? planDigest(current) !== savedDigest : false;
  const selectedPage = current?.pages.find((page) => page.id === current.selectedPageId) ?? current?.pages[0] ?? null;
  const selectedOverlay = selectedPage?.overlays.find((overlay) => overlay.id === current?.selectedOverlayId) ?? null;
  const modalOpen = signatureOpen || settingsOpen || !!confirmation;
  const blocked = modalOpen || !!busy;
  const closeState = useRef({ dirty, blocked });
  closeState.current = { dirty, blocked };
  const isDesktop = "__TAURI_INTERNALS__" in window;

  useEffect(() => setZoom(preferences.defaultZoom), [preferences.defaultZoom]);

  const settleDecision = useCallback((accepted: boolean) => {
    const resolve = decision.current;
    decision.current = null;
    setConfirmation(null);
    resolve?.(accepted);
  }, []);
  const askDiscard = useCallback((action: "open" | "demo" | "close"): Promise<boolean> => {
    if (decision.current) return Promise.resolve(false);
    setConfirmation({
      title: action === "close" ? "Close Folio?" : "Discard unsaved changes?",
      description: action === "close"
        ? "Your changes haven't been saved. Keep editing to save a copy, or discard them and close Folio."
        : `Your changes haven't been saved. Opening ${action === "demo" ? "the demo" : "another PDF"} will discard them.`,
      confirmLabel: action === "close" ? "Discard and close" : "Discard and open",
    });
    return new Promise((resolve) => { decision.current = resolve; });
  }, []);
  useEffect(() => () => { decision.current?.(false); decision.current = null; }, []);

  const closeSources = useCallback(async (using: FolioAdapter) => {
    const ids = [...openedSources.current];
    openedSources.current.clear();
    clearRenderCache(ids);
    await Promise.allSettled(ids.map((sourceId) => using.closeDocument(sourceId)));
  }, []);

  const navigateToPage = useCallback((pageId: string) => {
    setHistory((value) => value ? { ...value, present: { ...value.present, selectedPageId: pageId, selectedOverlayId: null } } : value);
    setNavigationRequest((request) => ({ pageId, revision: (request?.revision ?? 0) + 1 }));
  }, []);
  const selectPage = useCallback((pageId: string) => {
    setHistory((value) => value && value.present.selectedPageId !== pageId
      ? { ...value, present: { ...value.present, selectedPageId: pageId, selectedOverlayId: null } } : value);
  }, []);
  const selectOverlay = useCallback((pageId: string, id: string | null) => {
    setHistory((value) => value ? { ...value, present: { ...value.present, selectedPageId: pageId, selectedOverlayId: id } } : value);
  }, []);

  const installDocument = useCallback((info: Awaited<ReturnType<FolioAdapter["openPdf"]>>, nextAdapter: FolioAdapter, append = false) => {
    if (!info) return;
    openedSources.current.add(info.id);
    const newPages = documentToPages(info);
    setAdapter(nextAdapter);
    setHistory((existing) => {
      const document: EditorDocument = append && existing
        ? { ...existing.present, pages: [...existing.present.pages, ...newPages], selectedPageId: newPages[0]?.id ?? existing.present.selectedPageId, selectedOverlayId: null }
        : { name: info.name, pages: newPages, selectedPageId: newPages[0]?.id ?? null, selectedOverlayId: null };
      return append && existing ? commit(existing, () => document) : createHistory(document);
    });
    if (!append) {
      setSavedDigest(planDigest({ name: info.name, pages: newPages, selectedPageId: newPages[0]?.id ?? null, selectedOverlayId: null }));
      setZoom(preferences.defaultZoom);
    }
    if (newPages[0]) setNavigationRequest((request) => ({ pageId: newPages[0].id, revision: (request?.revision ?? 0) + 1 }));
    setTool("select"); setPendingSignature(null); setFailure(null);
    setNotice(append ? `${info.pages.length} pages added` : null);
  }, [preferences.defaultZoom]);

  const openDemo = useCallback(async () => {
    if (operation.current || modalOpen) return;
    operation.current = true;
    try {
      if (dirty && !await askDiscard("demo")) return;
      setBusy("Opening demo…");
      const demo = createDemoAdapter();
      const info = await demo.openPdf();
      if (!info) return;
      await closeSources(adapter);
      installDocument(info, demo);
    } catch (error) { setFailure(`Couldn’t open demo: ${errorMessage(error)}`); }
    finally { operation.current = false; setBusy(null); }
  }, [adapter, askDiscard, closeSources, dirty, installDocument, modalOpen]);
  useEffect(() => { if (initialDemo && !history) void openDemo(); }, []); // explicit demo entry only

  const openPdf = useCallback(async () => {
    if (operation.current || modalOpen) return;
    operation.current = true;
    try {
      if (dirty && !await askDiscard("open")) return;
      setBusy("Opening PDF…"); setFailure(null);
      const info = await nativeAdapter.openPdf();
      if (!info) return;
      await closeSources(adapter);
      installDocument(info, nativeAdapter);
    } catch (error) { setFailure(`Couldn’t open PDF: ${errorMessage(error)}`); }
    finally { operation.current = false; setBusy(null); }
  }, [adapter, askDiscard, closeSources, dirty, installDocument, modalOpen]);

  const addPdf = useCallback(async () => {
    if (operation.current || modalOpen || !current || adapter.kind !== "native") return;
    operation.current = true;
    setBusy("Adding PDF…"); setFailure(null);
    try { installDocument(await nativeAdapter.openPdf(), nativeAdapter, true); }
    catch (error) { setFailure(`Couldn’t add PDF: ${errorMessage(error)}`); }
    finally { operation.current = false; setBusy(null); }
  }, [adapter.kind, current, installDocument, modalOpen]);

  const exportPdf = useCallback(async () => {
    if (operation.current || modalOpen || !current?.pages.length) return;
    operation.current = true;
    setBusy(adapter.kind === "demo" ? "Preparing demo plan…" : "Exporting PDF…"); setFailure(null);
    try {
      const path = await adapter.exportPdf(current.pages);
      if (path) { setSavedDigest(planDigest(current)); setNotice(adapter.kind === "demo" ? "Demo edit plan downloaded" : `Saved a copy to ${path}`); }
    } catch (error) { setFailure(`Couldn’t export: ${errorMessage(error)}`); }
    finally { operation.current = false; setBusy(null); }
  }, [adapter, current, modalOpen]);

  const edit = useCallback((update: (document: EditorDocument) => EditorDocument) => {
    if (blocked) return;
    setHistory((value) => value ? commit(value, update) : value);
  }, [blocked]);
  const addText = (pageId: string, point: InkPoint) => {
    const id = uniqueId("text");
    edit((document) => ({
      ...document, selectedPageId: pageId, selectedOverlayId: id,
      pages: document.pages.map((page) => page.id === pageId
        ? { ...page, overlays: [...page.overlays, { type: "text", id, x: point.x, y: point.y, text: "Type here", fontSize: 18, color: "#2D2A26" }] } : page),
    }));
    setTool("select");
  };
  const addDrawing = (pageId: string, path: InkPoint[]) => {
    if (path.length < 2) return;
    const id = uniqueId("drawing");
    edit((document) => ({
      ...document, selectedPageId: pageId, selectedOverlayId: null,
      pages: document.pages.map((page) => page.id === pageId
        ? { ...page, overlays: [...page.overlays, { type: "ink", id, paths: [path], color: preferences.penColor, strokeWidth: preferences.penWidth }] } : page),
    }));
    setNotice("Drawing added");
  };
  const placeSignature = (pageId: string, point: InkPoint) => {
    const page = current?.pages.find((value) => value.id === pageId);
    if (!page || !pendingSignature) return;
    const paths = placeInkPaths(pendingSignature, point, page.width, page.height);
    const id = uniqueId("signature");
    edit((document) => ({ ...document, selectedPageId: pageId, selectedOverlayId: id,
      pages: document.pages.map((value) => value.id === pageId
        ? { ...value, overlays: [...value.overlays, { type: "ink", id, paths, color: preferences.penColor, strokeWidth: preferences.penWidth }] } : value) }));
    setPendingSignature(null); setTool("select"); setNotice("Signature placed");
  };
  const moveOverlay = (pageId: string, id: string, x: number, y: number, phase: "start" | "move" | "end") => {
    if (!history || (blocked && phase !== "end")) return;
    if (phase === "start") { dragOverlay.current = { document: history.present, pageId, x, y, id }; return; }
    const start = dragOverlay.current;
    if (!start || start.id !== id || start.pageId !== pageId) return;
    if (phase === "end") {
      setHistory((value) => value && planDigest(value.present) !== planDigest(start.document)
        ? { past: [...value.past, start.document], present: value.present, future: [] } : value);
      dragOverlay.current = null;
      return;
    }
    const dx = x - start.x, dy = y - start.y;
    const sourceOverlay = start.document.pages.find((page) => page.id === pageId)?.overlays.find((overlay) => overlay.id === id);
    if (!sourceOverlay) return;
    setHistory((value) => value ? { ...value, present: updateOverlay(value.present, pageId, id, () => sourceOverlay.type === "text"
      ? { ...sourceOverlay, x: sourceOverlay.x + dx, y: sourceOverlay.y + dy }
      : { ...sourceOverlay, paths: sourceOverlay.paths.map((path) => path.map((point) => ({ x: point.x + dx, y: point.y + dy }))) }) } : value);
  };

  const chooseTool = (next: Tool) => {
    setTool(next); setPendingSignature(null);
    if (selectedPage) selectOverlay(selectedPage.id, null);
    if (next === "signature") setSignatureOpen(true);
  };
  const changeHistory = useCallback((direction: "undo" | "redo") => {
    if (blocked) return;
    setHistory((value) => value ? (direction === "undo" ? undo(value) : redo(value)) : value);
  }, [blocked]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (blocked || decision.current) return;
      const command = event.ctrlKey || event.metaKey;
      const key = event.key.toLowerCase();
      const typing = (event.target as HTMLElement)?.closest?.("input, textarea, select, [contenteditable='true']");
      if (command && key === "o") { event.preventDefault(); void openPdf(); }
      else if (command && key === "s") { event.preventDefault(); void exportPdf(); }
      else if (command && !typing && key === "z") { event.preventDefault(); changeHistory(event.shiftKey ? "redo" : "undo"); }
      else if (command && !typing && key === "y") { event.preventDefault(); changeHistory("redo"); }
      else if ((event.key === "Delete" || event.key === "Backspace") && !typing && selectedPage && selectedOverlay) {
        event.preventDefault(); edit((document) => removeOverlay(document, selectedPage.id, selectedOverlay.id));
      } else if (event.key === "Escape" && !typing) {
        setTool("select"); setPendingSignature(null);
        if (selectedPage) selectOverlay(selectedPage.id, null);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [blocked, changeHistory, edit, exportPdf, openPdf, selectOverlay, selectedOverlay, selectedPage]);

  useEffect(() => {
    if (!isDesktop) {
      const beforeUnload = (event: BeforeUnloadEvent) => {
        if (closeState.current.dirty) { event.preventDefault(); event.returnValue = ""; }
      };
      window.addEventListener("beforeunload", beforeUnload);
      return () => window.removeEventListener("beforeunload", beforeUnload);
    }
    let unlisten: undefined | (() => void);
    let cancelled = false;
    import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().onCloseRequested(async (event) => {
      if (operation.current || closeState.current.blocked) { event.preventDefault(); return; }
      if (!closeState.current.dirty) return;
      operation.current = true;
      try { if (!await askDiscard("close")) event.preventDefault(); }
      finally { operation.current = false; }
    })).then((dispose) => { if (cancelled) dispose(); else unlisten = dispose; }).catch((error) => {
      setFailure(`Couldn’t enable close confirmation: ${errorMessage(error)}`);
    });
    return () => { cancelled = true; unlisten?.(); };
  }, [askDiscard, isDesktop]);

  const dragSide = (event: React.DragEvent<HTMLElement>): "before" | "after" => {
    const rect = event.currentTarget.getBoundingClientRect();
    return rect.height === 0 || event.clientY < rect.top + rect.height / 2 ? "before" : "after";
  };
  const dragOverPage = (event: React.DragEvent<HTMLButtonElement>, pageId: string) => {
    if (!draggedPageId || blocked) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    setDropTarget(draggedPageId === pageId ? null : { pageId, side: dragSide(event) });
    const list = event.currentTarget.closest<HTMLElement>(".thumbnails");
    if (list) {
      const rect = list.getBoundingClientRect();
      if (event.clientY < rect.top + 28) list.scrollTop -= 12;
      else if (event.clientY > rect.bottom - 28) list.scrollTop += 12;
    }
  };
  const dropPage = (event: React.DragEvent<HTMLButtonElement>, targetId: string) => {
    event.preventDefault();
    const sourceId = draggedPageId;
    const side = dragSide(event);
    setDraggedPageId(null); setDropTarget(null);
    if (!sourceId || sourceId === targetId || blocked) return;
    edit((document) => {
      const sourceIndex = document.pages.findIndex((page) => page.id === sourceId);
      const targetIndex = document.pages.findIndex((page) => page.id === targetId);
      if (sourceIndex < 0 || targetIndex < 0) return document;
      let destination = targetIndex + (side === "after" ? 1 : 0);
      if (sourceIndex < destination) destination -= 1;
      return destination === sourceIndex ? document : movePage(document, sourceId, destination);
    });
  };
  const selectedIndex = selectedPage ? current?.pages.findIndex((page) => page.id === selectedPage.id) ?? -1 : -1;
  const displaySize = selectedPage ? displayDimensions(selectedPage.width, selectedPage.height, selectedPage.rotation) : null;
  const dialogs = <>
    {signatureOpen && <SignaturePad onCancel={() => { setSignatureOpen(false); setTool("select"); }} onAccept={(paths) => { setSignatureOpen(false); setPendingSignature(paths); setNotice("Click a page to place your signature"); }}/>}
    {settingsOpen && <SettingsDialog preferences={preferences} onChange={setPreferences} onReset={resetPreferences} onClose={() => setSettingsOpen(false)}/>}
    {confirmation && <ConfirmDialog {...confirmation} onConfirm={() => settleDecision(true)} onCancel={() => settleDecision(false)}/>}
  </>;

  if (!current) return <main className="empty-shell">
    <div className="empty-brand"><span className="folio-mark">F</span><span>Folio</span></div>
    <button className="icon-button empty-settings" aria-label="Settings" onClick={() => setSettingsOpen(true)}><Settings size={20}/></button>
    <section className="empty-card">
      <div className="empty-illustration" aria-hidden="true"><div className="sheet sheet-back"/><div className="sheet sheet-front"><span/><span/><i/></div></div>
      <p className="eyebrow">A PRIVATE PDF WORKSPACE</p>
      <h1>Make PDFs feel finished.</h1>
      <p className="empty-copy">Arrange pages, add text, draw, and place your signature. Your documents stay on this device.</p>
      <div className="empty-actions"><button className="button primary large" disabled={!!busy} onClick={openPdf}><FolderOpen size={18}/> Open a PDF</button><button className="button large" disabled={blocked} onClick={openDemo}>Explore demo</button></div>
      {busy && <p className="busy-note">{busy}</p>}{failure && <p className="error-note" role="alert">{failure}</p>}
      <p className="privacy-note"><span>●</span> Local editing · Source files remain untouched</p>
    </section>{dialogs}
  </main>;

  return <div className="app-shell">
    <header className="titlebar">
      <div className="brand"><span className="folio-mark small">F</span><span>Folio</span></div>
      <div className="document-title"><span>{current.name}</span>{dirty && <i aria-label="Unsaved changes"/>}{adapter.kind === "demo" && <b>DEMO</b>}</div>
      <div className="titlebar-actions"><button className="icon-button" aria-label="Settings" title="Settings" disabled={!!busy} onClick={() => setSettingsOpen(true)}><Settings size={18}/></button></div>
    </header>
    <div className="toolbar" role="toolbar" aria-label="Document tools">
      <div className="tool-group"><button className="tool-button" onClick={openPdf} disabled={!!busy} title="Open PDF (Ctrl+O)"><FolderOpen size={17}/><span>Open</span></button><button className="tool-button" onClick={addPdf} disabled={blocked || adapter.kind === "demo"} title="Add another PDF"><FilePlus2 size={17}/><span>Add PDF</span></button></div>
      <div className="separator"/>
      <div className="tool-group modes">
        <button className={`tool-button ${tool === "select" ? "active" : ""}`} disabled={blocked} onClick={() => chooseTool("select")}><MousePointer2 size={17}/><span>Select</span></button>
        <button className={`tool-button ${tool === "text" ? "active" : ""}`} disabled={blocked} onClick={() => chooseTool("text")}><Type size={17}/><span>Text</span></button>
        <button className={`tool-button ${tool === "draw" ? "active" : ""}`} disabled={blocked} onClick={() => chooseTool("draw")}><Pencil size={17}/><span>Draw</span></button>
        <button className={`tool-button ${tool === "signature" ? "active" : ""}`} disabled={!!busy} onClick={() => chooseTool("signature")}><PenLine size={17}/><span>Signature</span></button>
      </div>
      <div className="separator"/>
      <div className="tool-group"><button className="icon-button" aria-label="Undo" disabled={blocked || !history?.past.length} onClick={() => changeHistory("undo")}><Undo2 size={18}/></button><button className="icon-button" aria-label="Redo" disabled={blocked || !history?.future.length} onClick={() => changeHistory("redo")}><Redo2 size={18}/></button></div>
      <div className="toolbar-spacer"/>
      <div className="zoom-control"><button aria-label="Zoom out" disabled={blocked} onClick={() => setZoom((value) => Math.max(50, value - 10))}><Minus size={15}/></button><button className="zoom-value" title="Reset zoom to 100% (Ctrl+scroll to zoom)" disabled={blocked} onClick={() => setZoom(100)}>{zoom}%</button><button aria-label="Zoom in" disabled={blocked} onClick={() => setZoom((value) => Math.min(200, value + 10))}><Plus size={15}/></button></div>
      <button className="button primary export" onClick={exportPdf} disabled={blocked}><Download size={17}/>{adapter.kind === "demo" ? "Export demo plan" : "Save a copy"}</button>
    </div>
    <div className="workspace">
      <aside className="sidebar">
        <div className="panel-heading"><span>PAGES</span><em>{current.pages.length}</em></div>
        <div className="thumbnails">
          {current.pages.map((page, index) => <button key={page.id} className={`thumbnail-item ${page.id === selectedPage?.id ? "selected" : ""} ${draggedPageId === page.id ? "dragging" : ""} ${dropTarget?.pageId === page.id ? `drop-${dropTarget.side}` : ""}`} aria-label={`Page ${index + 1} of ${current.pages.length}`} title="Drag to reorder pages" draggable={!blocked} disabled={blocked}
            onDragStart={(event) => { event.dataTransfer.setData("text/plain", page.id); event.dataTransfer.effectAllowed = "move"; setDraggedPageId(page.id); selectPage(page.id); }}
            onDragOver={(event) => dragOverPage(event, page.id)} onDrop={(event) => dropPage(event, page.id)} onDragEnd={() => { setDraggedPageId(null); setDropTarget(null); }}
            onDragLeave={(event) => { if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setDropTarget(null); }} onClick={() => navigateToPage(page.id)}>
            <span className="drag-handle"><GripVertical size={14}/></span><div className="thumbnail-paper"><Thumbnail adapter={adapter} page={page}/></div><span className="page-number">{index + 1}</span>
          </button>)}
        </div>
        <button className="add-pages" onClick={addPdf} disabled={blocked || adapter.kind === "demo"}><Plus size={16}/> Add pages</button>
      </aside>
      <div className="document-area">
        {pendingSignature && <div className="placement-banner"><PenLine size={16}/> Click a page to place your signature <button aria-label="Cancel signature placement" onClick={() => { setPendingSignature(null); setTool("select"); }}><X size={15}/></button></div>}
        <DocumentViewport adapter={adapter} pages={current.pages} selectedPageId={selectedPage?.id ?? null} selectedOverlayId={current.selectedOverlayId}
          zoom={zoom} onZoomChange={setZoom} viewMode={preferences.viewMode} tool={tool} penColor={preferences.penColor} penWidth={preferences.penWidth}
          pendingSignature={pendingSignature} interactionDisabled={blocked} navigationRequest={navigationRequest}
          onSelectPage={selectPage} onSelectOverlay={selectOverlay} onAddText={addText} onPlaceSignature={placeSignature} onMoveOverlay={moveOverlay} onDraw={addDrawing}/>
        <div className="page-nav"><button aria-label="Previous page" disabled={blocked || selectedIndex <= 0} onClick={() => navigateToPage(current.pages[selectedIndex - 1].id)}><ChevronLeft size={16}/></button><span>Page {selectedIndex + 1} of {current.pages.length}</span><button aria-label="Next page" disabled={blocked || selectedIndex >= current.pages.length - 1} onClick={() => navigateToPage(current.pages[selectedIndex + 1].id)}><ChevronRight size={16}/></button></div>
      </div>
      <aside className="properties">
        <div className="panel-heading"><span>PROPERTIES</span></div>
        {tool === "draw" ? <section className="property-section"><h3>Draw</h3>
          <label className="field-label">Pen color<span className="color-input"><input aria-label="Pen color" type="color" value={preferences.penColor} onChange={(event) => setPreferences({ ...preferences, penColor: event.target.value.toUpperCase() })}/><code>{preferences.penColor}</code></span></label>
          <label className="field-label">Pen width · {preferences.penWidth} pt<input aria-label="Pen width" type="range" min="0.5" max="20" step="0.5" value={preferences.penWidth} onChange={(event) => setPreferences({ ...preferences, penWidth: Number(event.target.value) })}/></label>
          <p>Draw directly on any page. Each stroke can be undone. Switch to Select to move or delete a stroke.</p>
        </section> : selectedOverlay && selectedPage ? <OverlayProperties overlay={selectedOverlay} onChange={(update) => edit((document) => updateOverlay(document, selectedPage.id, selectedOverlay.id, update))} onDelete={() => edit((document) => removeOverlay(document, selectedPage.id, selectedOverlay.id))}/> : selectedPage && <>
          <section className="property-section"><h3>Page</h3><div className="page-summary"><div className="mini-page" style={{ aspectRatio: `${displaySize!.width}/${displaySize!.height}` }}/><div><strong>Page {selectedIndex + 1}</strong><span>{Math.round(selectedPage.width)} × {Math.round(selectedPage.height)} pt</span><small>{selectedPage.rotation ? `${selectedPage.rotation}° clockwise` : "Original orientation"}</small></div></div></section>
          <section className="property-section"><h3>Arrange</h3><div className="property-grid"><button disabled={blocked || selectedIndex <= 0} onClick={() => edit((document) => movePage(document, selectedPage.id, selectedIndex - 1))}><ArrowUp size={16}/>Move up</button><button disabled={blocked || selectedIndex >= current.pages.length - 1} onClick={() => edit((document) => movePage(document, selectedPage.id, selectedIndex + 1))}><ArrowDown size={16}/>Move down</button><button disabled={blocked} onClick={() => edit((document) => rotatePage(document, selectedPage.id))}><RotateCw size={16}/>Rotate</button><button disabled={blocked} onClick={() => { const id = uniqueId("page"); edit((document) => duplicatePage(document, selectedPage.id, id)); setNavigationRequest((request) => ({ pageId: id, revision: (request?.revision ?? 0) + 1 })); }}><Copy size={16}/>Duplicate</button></div></section>
          <section className="property-section"><h3>Page actions</h3><button className="danger-action" disabled={blocked || current.pages.length <= 1} onClick={() => edit((document) => deletePage(document, selectedPage.id))}><Trash2 size={16}/>Delete page</button><p>Source files are never changed.</p></section>
        </>}
      </aside>
    </div>
    <footer className="statusbar"><span>{busy ?? notice ?? (dirty ? "Unsaved edits" : "All changes exported")}</span>{failure && <span className="status-error" role="alert">{failure}</span>}<span className="status-spacer"/><span>{selectedPage ? `${Math.round(selectedPage.width)} × ${Math.round(selectedPage.height)} pt` : ""}</span><span>Local only</span></footer>
    {dialogs}
  </div>;
}

function OverlayProperties({ overlay, onChange, onDelete }: { overlay: Overlay; onChange(update: (value: Overlay) => Overlay): void; onDelete(): void }) {
  return <>
    <section className="property-section"><h3>{overlay.type === "text" ? "Text" : "Ink"}</h3>{overlay.type === "text" ? <>
      <label className="field-label">Content<textarea value={overlay.text} rows={4} onChange={(event) => onChange((value) => value.type === "text" ? { ...value, text: event.target.value } : value)}/></label>
      <div className="field-row"><label className="field-label">Size<input type="number" min="6" max="96" value={overlay.fontSize} onChange={(event) => onChange((value) => value.type === "text" ? { ...value, fontSize: Math.max(6, Math.min(96, Number(event.target.value))) } : value)}/></label><label className="field-label">Color<span className="color-input"><input type="color" value={overlay.color} onChange={(event) => onChange((value) => ({ ...value, color: event.target.value.toUpperCase() }))}/><code>{overlay.color}</code></span></label></div>
      <p>Helvetica · {overlay.text.split("\n").length} {overlay.text.includes("\n") ? "lines" : "line"}</p>
    </> : <><label className="field-label">Ink color<span className="color-input"><input type="color" value={overlay.color} onChange={(event) => onChange((value) => ({ ...value, color: event.target.value.toUpperCase() }))}/><code>{overlay.color}</code></span></label><label className="field-label">Stroke width · {overlay.strokeWidth} pt<input type="range" min="0.5" max="20" step="0.5" value={overlay.strokeWidth} onChange={(event) => onChange((value) => value.type === "ink" ? { ...value, strokeWidth: Number(event.target.value) } : value)}/></label></>}</section>
    <section className="property-section"><h3>Position</h3><p>Drag this item directly on the page to move it.</p><button className="danger-action" onClick={onDelete}><Trash2 size={16}/>Delete {overlay.type === "text" ? "text" : "ink"}</button></section>
  </>;
}
