import { useCallback, useEffect, useRef, useState, type SetStateAction } from "react";
import {
  ArrowDown, ArrowUp, ChevronLeft, ChevronRight, Copy, FilePlus2, FolderOpen,
  GripVertical, Highlighter, MessageSquare, Search, Printer, Minus, MousePointer2, PenLine, Pencil, Plus, Redo2, RotateCw, Settings, Trash2, Type, TextCursorInput, Undo2, X,
} from "lucide-react";
import { createDemoAdapter, documentToPages, nativeAdapter, type FolioAdapter } from "./editor/adapter";
import { displayDimensions, placeInkPaths } from "./editor/geometry";
import {
  commit, deletePage, duplicatePage, movePage, planDigest, redo, removeOverlay,
  rotatePage, undo, uniqueId, updateOverlay, type EditorDocument, type History, type Overlay,
} from "./editor/model";
import { addSession, createSession, emptyWorkspace, removeSession, updateSession, type DocumentSession, type ScrollPosition } from "./editor/workspace";
import type { AnnotationRect, InkPoint } from "./editor/types";
import { usePreferences } from "./editor/preferences";
import { clearRenderCache, Thumbnail } from "./components/PageView";
import { DocumentViewport } from "./components/DocumentViewport";
import { SignaturePad } from "./components/SignaturePad";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { SettingsDialog } from "./components/SettingsDialog";
import {useWorkspaceRecovery} from "./editor/useWorkspaceRecovery";
import {useDocumentSearch} from "./editor/search";
import {SearchBar} from "./components/SearchBar";
import {PrintDialog} from "./components/PrintDialog";
import {RecoveryDialog} from "./components/RecoveryDialog";
import {printPdf,type PrintOptions} from "./editor/printing";
import type {PagePlan} from "./editor/types";
import "./styles.css";
import {SaveCopyButton} from "./components/SaveCopyButton";
import { OverlayProperties } from "./components/OverlayProperties";
import { ExistingTextDialog } from "./components/ExistingTextDialog";
import { commitTextReplacement } from "./editor/text-edit";
import type {EditableTextRun} from "./editor/types";
import { ReviewList } from "./components/ReviewList";

interface AppProps { initialDemo?: boolean }
type Tool = "edit" | "select" | "text" | "signature" | "draw" | "highlight" | "comment";
type Confirmation = { title: string; description: string; confirmLabel: string };
function errorMessage(error: unknown) { return error instanceof Error ? error.message : String(error); }

export function App({ initialDemo = new URLSearchParams(location.search).get("demo") === "1" }: AppProps) {
  const { preferences, setPreferences, resetPreferences } = usePreferences();
  const [workspace, setWorkspace] = useState(emptyWorkspace);
  const workspaceCurrent = useRef(workspace); workspaceCurrent.current = workspace;
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const activeTab = workspace.tabs.find(tab => tab.id === workspace.activeId) ?? null;
  const history = activeTab?.history ?? null;
  const savedDigest = activeTab?.savedDigest ?? "";
  const adapter = activeTab?.adapter ?? nativeAdapter;
  const zoom = activeTab?.zoom ?? preferences.defaultZoom;
  const navigationRequest = activeTab?.navigationRequest ?? null;
  const scrollPositions = useRef(new Map<string, ScrollPosition>());
  const isDesktop = "__TAURI_INTERNALS__" in window;
  const recovery = useWorkspaceRecovery(isDesktop,workspace,setWorkspace,scrollPositions);
  const recoveryCurrent = useRef(recovery); recoveryCurrent.current = recovery;
  const [printOpen,setPrintOpen] = useState(false);
  const [searchTabs,setSearchTabs] = useState<Record<string,{open:boolean;query:string;index:number}>>({});
  const searchState = searchTabs[workspace.activeId??""]??{open:false,query:"",index:0};
  const search = useDocumentSearch(adapter,history?.present.pages??[],searchState.query,searchState.open);
  const searchMatch = search.matches[Math.min(searchState.index,search.matches.length-1)];
  const updateSearch = useCallback((patch:Partial<typeof searchState>)=>{
    if(!workspace.activeId)return;
    setSearchTabs(value=>({...value,[workspace.activeId!]:{...(value[workspace.activeId!]??{open:false,query:"",index:0}),...patch}}));
  },[workspace.activeId]);
  const [newTextId, setNewTextId] = useState<string | null>(null);
  const setHistory = useCallback((action: SetStateAction<History<EditorDocument> | null>) => {
    setWorkspace(value => updateSession(value, workspace.activeId, tab => {
      const next = typeof action === "function" ? action(tab.history) : action;
      return next && next !== tab.history ? {...tab, history:next} : tab;
    }));
  }, [workspace.activeId]);
  const setSavedDigest = useCallback((digest: string) => setWorkspace(value => updateSession(value, workspace.activeId, tab => ({...tab,savedDigest:digest}))), [workspace.activeId]);
  const setZoom = useCallback((action: SetStateAction<number>) => setWorkspace(value => updateSession(value, workspace.activeId, tab => {
    const next = typeof action === "function" ? action(tab.zoom) : action;
    return next === tab.zoom ? tab : {...tab,zoom:next};
  })), [workspace.activeId]);
  const setNavigationRequest = useCallback((action: SetStateAction<DocumentSession["navigationRequest"]>) => setWorkspace(value => updateSession(value, workspace.activeId, tab => ({...tab,navigationRequest:typeof action === "function" ? action(tab.navigationRequest) : action}))), [workspace.activeId]);
  const [tool, setTool] = useState<Tool>("select");
  const [pendingSignature, setPendingSignature] = useState<InkPoint[][] | null>(null);
  const [textEdit,setTextEdit] = useState<{tabId:string;page:PagePlan;run:EditableTextRun}|null>(null);
  const [textEditError,setTextEditError] = useState<string|null>(null);
  const [signatureOpen, setSignatureOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [draggedPageId, setDraggedPageId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<{ pageId: string; side: "before" | "after" } | null>(null);
  const dragOverlay = useRef<{ document: EditorDocument; pageId: string; x: number; y: number; id: string } | null>(null);
  const operation = useRef(false);
  const decision = useRef<((accepted: boolean) => void) | null>(null);
  const current = history?.present ?? null;
  const dirty = current ? planDigest(current) !== savedDigest : false;
  const selectedPage = current?.pages.find((page) => page.id === current.selectedPageId) ?? current?.pages[0] ?? null;
  const selectedOverlay = selectedPage?.overlays.find((overlay) => overlay.id === current?.selectedOverlayId) ?? null;
  const modalOpen = !!textEdit || signatureOpen || settingsOpen || printOpen || !!confirmation || !recovery.ready;
  const blocked = modalOpen || !!busy;
  const dirtyTabs = workspace.tabs.filter(tab => planDigest(tab.history.present) !== tab.savedDigest);
  const closeState = useRef({ dirty:dirtyTabs.length > 0, blocked, dirtyCount:dirtyTabs.length });
  closeState.current = { dirty:dirtyTabs.length > 0, blocked, dirtyCount:dirtyTabs.length };
  useEffect(() => setZoom(preferences.defaultZoom), [preferences.defaultZoom]);

  const settleDecision = useCallback((accepted: boolean) => {
    const resolve = decision.current;
    decision.current = null;
    setConfirmation(null);
    resolve?.(accepted);
  }, []);
  const askDiscard = useCallback((action: "tab" | "close", name?: string): Promise<boolean> => {
    if (decision.current) return Promise.resolve(false);
    setConfirmation({
      title: action === "close" ? "Close Folio?" : "Close document?",
      description: action === "close"
        ? "You have unsaved changes in " + closeState.current.dirtyCount + " document" + (closeState.current.dirtyCount === 1 ? "" : "s") + ". Keep editing to save your work, or discard the changes and close Folio."
        : "Changes to " + name + " haven't been saved. Keep editing to save a copy, or discard them and close this tab.",
      confirmLabel: "Discard and close",
    });
    return new Promise(resolve => { decision.current = resolve; });
  }, []);
  useEffect(() => () => { decision.current?.(false); decision.current = null; }, []);

  useEffect(()=>{
    if(searchState.open&&searchMatch){
      setHistory(value=>value?{...value,present:{...value.present,selectedPageId:searchMatch.pageId,selectedOverlayId:null}}:value);
      setNavigationRequest(request=>({pageId:searchMatch.pageId,rect:searchMatch.rects[0],revision:(request?.revision??0)+1}));
    }
  },[searchState.open,searchMatch?.id,workspace.activeId]);
  const openSearch=useCallback(()=>{
    if(blocked||!current)return;
    updateSearch({open:true});setTool("select");setPendingSignature(null);
    document.querySelector<HTMLInputElement>('[aria-label="Find in document"]')?.select();
  },[blocked,current,updateSearch]);
  const moveSearch=(direction:number)=>{if(search.matches.length)updateSearch({index:(Math.min(searchState.index,search.matches.length-1)+direction+search.matches.length)%search.matches.length});};
  const openPrint=useCallback(()=>{
    if(blocked||!current)return;
    if(adapter.kind!=="native"||!isDesktop){setFailure("Printing is available for PDFs in the Windows desktop app.");return;}
    setPrintOpen(true);
  },[blocked,current,adapter.kind,isDesktop]);
  const startPrint=async(pages:PagePlan[],options:PrintOptions)=>{
    if(operation.current)return;
    operation.current=true;setPrintOpen(false);setBusy("Preparing print job…");setFailure(null);
    try{const printed=await printPdf(pages,options);setNotice(printed?"Document sent to printer":"Printing cancelled");}
    catch(error){setFailure("Couldn’t print: "+errorMessage(error));}
    finally{operation.current=false;setBusy(null);}
  };
  const releaseSession = useCallback(async (tab: DocumentSession) => {
    clearRenderCache(tab.sourceIds);
    await Promise.allSettled(tab.sourceIds.map(sourceId => tab.adapter.closeDocument(sourceId)));
    scrollPositions.current.delete(tab.id);
  }, []);
  const clearTransient = useCallback(() => {
    setTool("select"); setPendingSignature(null); setNewTextId(null); setTextEdit(null); setTextEditError(null);
    setNotice(null); setFailure(null); setDraggedPageId(null); setDropTarget(null);
    window.getSelection()?.removeAllRanges();
  }, []);
  const switchTab = useCallback((id: string) => {
    if (operation.current || blocked || id === workspace.activeId) return;
    const start = dragOverlay.current;
    if (start) {
      setHistory(value => value && planDigest(value.present) !== planDigest(start.document)
        ? {past:[...value.past,start.document],present:value.present,future:[]} : value);
      dragOverlay.current = null;
    }
    setWorkspace(value => value.tabs.some(tab => tab.id === id) ? {...value,activeId:id} : value);
    clearTransient();
  }, [blocked, clearTransient, setHistory, workspace.activeId]);
  const closeTab = useCallback(async (id: string) => {
    if (operation.current || blocked) return;
    const tab = workspace.tabs.find(value => value.id === id);
    if (!tab) return;
    operation.current = true;
    try {
      if (planDigest(tab.history.present) !== tab.savedDigest && !await askDiscard("tab", tab.history.present.name)) return;
      setBusy("Closing document…");
      const remaining=removeSession(workspace,id);
      await recovery.checkpoint(remaining);
      setWorkspace(remaining);
      await releaseSession(tab);
      setSearchTabs(value=>{const next={...value};delete next[id];return next;});
      if (id === workspace.activeId) { dragOverlay.current = null; clearTransient(); }
    } catch(error){setFailure("Couldn’t close document: "+errorMessage(error));}
    finally { operation.current = false; setBusy(null); }
  }, [askDiscard, blocked, clearTransient, releaseSession, workspace,recovery.checkpoint]);

  const navigateToPage = useCallback((pageId: string) => {
    setHistory((value) => value ? { ...value, present: { ...value.present, selectedPageId: pageId, selectedOverlayId: null } } : value);
    setNavigationRequest((request) => ({ pageId, revision: (request?.revision ?? 0) + 1 }));
  }, [setHistory, setNavigationRequest]);
  const selectPage = useCallback((pageId: string) => {
    setHistory((value) => value && value.present.selectedPageId !== pageId
      ? { ...value, present: { ...value.present, selectedPageId: pageId, selectedOverlayId: null } } : value);
  }, [setHistory]);
  const selectOverlay = useCallback((pageId: string, id: string | null) => {
    setHistory((value) => value ? { ...value, present: { ...value.present, selectedPageId: pageId, selectedOverlayId: id } } : value);
  }, [setHistory]);

  const installDocument = useCallback((info: Awaited<ReturnType<FolioAdapter["openPdf"]>>, nextAdapter: FolioAdapter, append = false) => {
    if (!info) return;
    if (append && activeTab) {
      const newPages = documentToPages(info);
      setWorkspace(value => updateSession(value, activeTab.id, tab => ({
        ...tab, sourceIds:[...new Set([...tab.sourceIds,info.id])],
        history:commit(tab.history, document => ({...document,pages:[...document.pages,...newPages],selectedPageId:newPages[0]?.id??document.selectedPageId,selectedOverlayId:null})),
        navigationRequest:newPages[0] ? {pageId:newPages[0].id,revision:(tab.navigationRequest?.revision??0)+1} : tab.navigationRequest,
      })));
      setNotice(info.pages.length + " pages added");
    } else {
      const session = createSession(info, nextAdapter, preferences.defaultZoom);
      setWorkspace(value => addSession(value,session));
      clearTransient();
    }
    setTool("select"); setPendingSignature(null); setFailure(null);
  }, [activeTab, clearTransient, preferences.defaultZoom]);
  const openDemo = useCallback(async () => {
    if (operation.current || modalOpen) return;
    operation.current = true;
    try {
      setBusy("Opening demo…");
      const demo = createDemoAdapter();
      installDocument(await demo.openPdf(), demo);
    } catch (error) { setFailure("Couldn’t open demo: " + errorMessage(error)); }
    finally { operation.current = false; setBusy(null); }
  }, [installDocument, modalOpen]);
  useEffect(() => { if (initialDemo && !history) void openDemo(); }, []);
  const openPdf = useCallback(async () => {
    if (operation.current || modalOpen) return;
    operation.current = true;
    try {
      setBusy("Opening PDF…"); setFailure(null);
      installDocument(await nativeAdapter.openPdf(), nativeAdapter);
    } catch (error) { setFailure("Couldn’t open PDF: " + errorMessage(error)); }
    finally { operation.current = false; setBusy(null); }
  }, [installDocument, modalOpen]);

  const addPdf = useCallback(async () => {
    if (operation.current || modalOpen || !current || adapter.kind !== "native") return;
    operation.current = true;
    setBusy("Adding PDF…"); setFailure(null);
    try { installDocument(await nativeAdapter.openPdf(), nativeAdapter, true); }
    catch (error) { setFailure(`Couldn’t add PDF: ${errorMessage(error)}`); }
    finally { operation.current = false; setBusy(null); }
  }, [adapter.kind, current, installDocument, modalOpen]);

  const exportPdf = useCallback(async (flatten = false) => {
    if (operation.current || modalOpen || !current?.pages.length) return;
    operation.current = true;
    setBusy(adapter.kind === "demo" ? "Preparing demo plan…" : "Exporting PDF…"); setFailure(null);
    try {
      const path = await (flatten ? adapter.exportPdf(current.pages, {flatten:true}) : adapter.exportPdf(current.pages));
      if (path) { if (!flatten) setSavedDigest(planDigest(current)); setNotice(adapter.kind === "demo" ? "Demo edit plan downloaded" : flatten ? `Exported flattened text and ink to ${path}` : `Saved a copy to ${path}`); }
    } catch (error) { setFailure(`Couldn’t export: ${errorMessage(error)}`); }
    finally { operation.current = false; setBusy(null); }
  }, [adapter, current, modalOpen, setSavedDigest]);

  const edit = useCallback((update: (document: EditorDocument) => EditorDocument) => {
    if (blocked) return;
    setHistory((value) => value ? commit(value, update) : value);
  }, [blocked, setHistory]);
  const beginTextEdit = (pageId:string, run:EditableTextRun) => {
    if (blocked || !activeTab || !adapter.replaceText) return;
    const page = current?.pages.find(page=>page.id===pageId);
    if (!page) return;
    setTextEdit({tabId:activeTab.id,page,run});setTextEditError(null);
  };
  const applyTextEdit = async (replacement:string) => {
    const request=textEdit;
    if (!request || !request.run.supported || operation.current || !adapter.replaceText) return;
    const editingAdapter=adapter;
    operation.current=true;setBusy("Updating PDF text…");setTextEditError(null);
    let orphanSource:string|null=null;
    try {
      const result=await editingAdapter.replaceText!(request.page.sourceId,request.page.pageIndex,request.run.objectIndex,request.run.text,replacement);
      const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
      if (result.id && !workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(result.id))) orphanSource=result.id;
      if (!mounted.current || !session) return;
      // Validate before scheduling state; blocked workspace controls keep this version stable.
      commitTextReplacement(session,request.page,result);
      setWorkspace(value=>updateSession(value,request.tabId,tab=>commitTextReplacement(tab,request.page,result)));
      orphanSource=null;setTextEdit(null);setTool("select");setNotice("PDF text updated");
    } catch (error) {
      if (mounted.current) setTextEditError(errorMessage(error));
    } finally {
      if (orphanSource) await editingAdapter.closeDocument(orphanSource).catch(()=>{});
      operation.current=false;if (mounted.current) setBusy(null);
    }
  };
  const addText = (pageId: string, point: InkPoint) => {
    const id = uniqueId("text");
    edit((document) => ({
      ...document, selectedPageId: pageId, selectedOverlayId: id,
      pages: document.pages.map((page) => page.id === pageId
        ? { ...page, overlays: [...page.overlays, { type: "text", id, x: point.x, y: point.y, text: "Type here", fontSize: 18, color: "#2D2A26" }] } : page),
    }));
    setNewTextId(id);
    setTool("select");
  };
  const addHighlight = (pageId: string, rects: AnnotationRect[]) => {
    if (!rects.length) return;
    const id = uniqueId("highlight");
    edit(document => ({...document, selectedPageId:pageId, selectedOverlayId:id,
      pages:document.pages.map(page=>page.id===pageId ? {...page,overlays:[...page.overlays,{type:"highlight",id,rects,color:"#FFE066"}]} : page)}));
    setNotice("Highlight added");
  };
  const addComment = (pageId: string, point: InkPoint) => {
    const id = uniqueId("comment");
    edit(document => ({...document, selectedPageId:pageId, selectedOverlayId:id,
      pages:document.pages.map(page=>page.id===pageId ? {...page,overlays:[...page.overlays,{type:"comment",id,x:Math.min(point.x,Math.max(0,page.width-24)),y:Math.min(point.y,Math.max(0,page.height-24)),text:"",color:"#FFE066"}]} : page)}));
    setNewTextId(id); setTool("select");
  };
  const navigateToAnnotation = (pageId: string, overlay: Overlay) => {
    if (blocked) return;
    setTool("select"); setPendingSignature(null); selectOverlay(pageId,overlay.id);
    const rect = overlay.type === "highlight" ? overlay.rects[0] : overlay.type === "comment" ? {x:overlay.x,y:overlay.y,width:24,height:24} : undefined;
    setNavigationRequest(request=>({pageId,rect,revision:(request?.revision??0)+1}));
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
    setHistory((value) => value ? { ...value, present: updateOverlay(value.present, pageId, id, () => (sourceOverlay.type === "text" || sourceOverlay.type === "comment")
      ? { ...sourceOverlay, x: sourceOverlay.x + dx, y: sourceOverlay.y + dy }
      : sourceOverlay.type === "highlight" ? sourceOverlay : { ...sourceOverlay, paths: sourceOverlay.paths.map((path) => path.map((point) => ({ x: point.x + dx, y: point.y + dy }))) }) } : value);
  };

  const chooseTool = (next: Tool) => {
    setTool(next); setPendingSignature(null);
    if (selectedPage) selectOverlay(selectedPage.id, null);
    if (next === "signature") setSignatureOpen(true);
  };
  const changeHistory = useCallback((direction: "undo" | "redo") => {
    if (blocked) return;
    setHistory((value) => value ? (direction === "undo" ? undo(value) : redo(value)) : value);
  }, [blocked, setHistory]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (blocked || decision.current) return;
      const command = event.ctrlKey || event.metaKey;
      const key = event.key.toLowerCase();
      const typing = (event.target as HTMLElement)?.closest?.("input, textarea, select, [contenteditable='true']");
      if (command && key === "w" && workspace.activeId) { event.preventDefault(); void closeTab(workspace.activeId); }
      else if (event.ctrlKey && event.key === "Tab" && workspace.tabs.length > 1) {
        event.preventDefault();
        const index = workspace.tabs.findIndex(tab => tab.id === workspace.activeId);
        switchTab(workspace.tabs[(index + (event.shiftKey ? -1 : 1) + workspace.tabs.length) % workspace.tabs.length].id);
      }
      else if (command && key === "o") { event.preventDefault(); void openPdf(); }
      else if (command && key === "f") { event.preventDefault(); openSearch(); }
      else if (command && key === "p") { event.preventDefault(); openPrint(); }
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
  }, [blocked, changeHistory, closeTab, edit, exportPdf, openPdf, openSearch, openPrint, selectOverlay, selectedOverlay, selectedPage, switchTab, workspace]);

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
      operation.current = true;
      try {
        if (closeState.current.dirty && !await askDiscard("close")) {event.preventDefault();return;}
        await recoveryCurrent.current.finishClose();
      } catch(error) {event.preventDefault();setFailure("Couldn’t clear recovery before closing: "+errorMessage(error));}
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
    {textEdit && <ExistingTextDialog run={textEdit.run} busy={!!busy} error={textEditError} onApply={replacement=>void applyTextEdit(replacement)} onCancel={()=>{if(!operation.current){setTextEdit(null);setTextEditError(null);}}}/> }
    {(recovery.pending||recovery.error)&&<RecoveryDialog count={recovery.pending?.tabs.length??0} error={recovery.error} busy={recovery.working} onRestore={()=>void recovery.restore()} onDiscard={()=>void recovery.discard()} onSkip={recovery.skip}/>}
    {printOpen&&current&&<PrintDialog pages={current.pages} currentPageId={selectedPage?.id??null} onClose={()=>setPrintOpen(false)} onPrint={(pages,options)=>void startPrint(pages,options)}/>}
    {signatureOpen && <SignaturePad onCancel={() => { setSignatureOpen(false); setTool("select"); }} onAccept={(paths) => { setSignatureOpen(false); setPendingSignature(paths); setNotice("Click a page to place your signature"); }}/>}
    {settingsOpen && <SettingsDialog preferences={preferences} onChange={setPreferences} onReset={resetPreferences} onClose={() => setSettingsOpen(false)}/>}
    {confirmation && <ConfirmDialog {...confirmation} onConfirm={() => settleDecision(true)} onCancel={() => settleDecision(false)}/>}
  </>;

  if (!current) return <main className="empty-shell">
    <div className="empty-brand"><span className="folio-mark">F</span><span>Folio</span></div>
    <button className="icon-button empty-settings" disabled={blocked} aria-label="Settings" onClick={() => setSettingsOpen(true)}><Settings size={20}/></button>
    <section className="empty-card">
      <div className="empty-illustration" aria-hidden="true"><div className="sheet sheet-back"/><div className="sheet sheet-front"><span/><span/><i/></div></div>
      <p className="eyebrow">A PRIVATE PDF WORKSPACE</p>
      <h1>Make PDFs feel finished.</h1>
      <p className="empty-copy">Arrange pages, add text, draw, and place your signature. Your documents stay on this device.</p>
      <div className="empty-actions"><button className="button primary large" disabled={blocked} onClick={openPdf}><FolderOpen size={18}/> Open a PDF</button><button className="button large" disabled={blocked} onClick={openDemo}>Explore demo</button></div>
      {recovery.warning&&<p className="error-note" role="status">{recovery.warning}</p>}{!recovery.ready&&!recovery.pending&&!recovery.error&&<p className="busy-note">Checking for recovered work…</p>}{busy && <p className="busy-note">{busy}</p>}{failure && <p className="error-note" role="alert">{failure}</p>}
      <p className="privacy-note"><span>●</span> Local editing · Source files remain untouched</p>
    </section>{dialogs}
  </main>;

  return <div className="app-shell">
    <header className="titlebar">
      <div className="brand"><span className="folio-mark small">F</span><span>Folio</span></div>
      <div className="document-tabs" role="tablist" aria-label="Open documents">
        {workspace.tabs.map(tab => <div key={tab.id} className={"document-tab " + (tab.id === workspace.activeId ? "active" : "")}>
          <button id={"tab-" + tab.id} role="tab" aria-label={tab.history.present.name} aria-selected={tab.id === workspace.activeId} tabIndex={tab.id === workspace.activeId ? 0 : -1} disabled={!!busy}
            title={tab.history.present.name} onClick={() => switchTab(tab.id)} onKeyDown={event => {
              if (!["ArrowLeft","ArrowRight","Home","End"].includes(event.key)) return;
              event.preventDefault();
              const index = workspace.tabs.indexOf(tab);
              const next = event.key === "Home" ? 0 : event.key === "End" ? workspace.tabs.length - 1 : (index + (event.key === "ArrowLeft" ? -1 : 1) + workspace.tabs.length) % workspace.tabs.length;
              switchTab(workspace.tabs[next].id);
              document.getElementById("tab-" + workspace.tabs[next].id)?.focus();
            }}>
            <span>{tab.history.present.name}</span>{planDigest(tab.history.present) !== tab.savedDigest && <i aria-label="Unsaved changes"/>}
          </button>
          <button className="tab-close" aria-label={"Close " + tab.history.present.name} title="Close tab (Ctrl+W)" disabled={!!busy} onClick={() => void closeTab(tab.id)}><X size={13}/></button>
        </div>)}
        <button className="icon-button new-document" aria-label="Open PDF in new tab" title="Open PDF in new tab (Ctrl+O)" disabled={blocked} onClick={openPdf}><Plus size={16}/></button>
      </div>
      <div className="titlebar-actions"><button className="icon-button" aria-label="Settings" title="Settings" disabled={!!busy} onClick={() => setSettingsOpen(true)}><Settings size={18}/></button></div>
    </header>
    <div className="toolbar" role="toolbar" aria-label="Document tools">
      <div className="tool-group"><button className="tool-button" onClick={openPdf} disabled={!!busy} title="Open PDF (Ctrl+O)"><FolderOpen size={17}/><span>Open</span></button><button className="tool-button" onClick={addPdf} disabled={blocked || adapter.kind === "demo"} title="Add another PDF"><FilePlus2 size={17}/><span>Add PDF</span></button></div>
      <div className="separator"/>
      <div className="tool-group modes">
        <button className={`tool-button ${tool === "select" ? "active" : ""}`} disabled={blocked} onClick={() => chooseTool("select")}><MousePointer2 size={17}/><span>Select</span></button>
        <button className={`tool-button ${tool === "edit" ? "active" : ""}`} disabled={blocked || !adapter.listTextRuns || !adapter.replaceText} onClick={() => chooseTool("edit")} title="Change supported existing PDF text"><TextCursorInput size={17}/><span>Edit text</span></button>
        <button className={`tool-button ${tool === "text" ? "active" : ""}`} disabled={blocked} onClick={() => chooseTool("text")}><Type size={17}/><span>Text</span></button>
        <button className={`tool-button ${tool === "draw" ? "active" : ""}`} disabled={blocked} onClick={() => chooseTool("draw")}><Pencil size={17}/><span>Draw</span></button>
        <button className={`tool-button ${tool === "signature" ? "active" : ""}`} disabled={!!busy} onClick={() => chooseTool("signature")}><PenLine size={17}/><span>Signature</span></button>
        <button className={`tool-button ${tool === "highlight" ? "active" : ""}`} disabled={blocked} onClick={() => chooseTool("highlight")}><Highlighter size={17}/><span>Highlight</span></button>
        <button className={`tool-button ${tool === "comment" ? "active" : ""}`} disabled={blocked} onClick={() => chooseTool("comment")}><MessageSquare size={17}/><span>Comment</span></button>
      </div>
      <div className="separator"/>
      <div className="tool-group"><button className="icon-button" aria-label="Undo" disabled={blocked || !history?.past.length} onClick={() => changeHistory("undo")}><Undo2 size={18}/></button><button className="icon-button" aria-label="Redo" disabled={blocked || !history?.future.length} onClick={() => changeHistory("redo")}><Redo2 size={18}/></button></div>
      <div className="toolbar-spacer"/>
      <div className="zoom-control"><button aria-label="Zoom out" disabled={blocked} onClick={() => setZoom((value) => Math.max(50, value - 10))}><Minus size={15}/></button><button className="zoom-value" title="Reset zoom to 100% (Ctrl+scroll to zoom)" disabled={blocked} onClick={() => setZoom(100)}>{zoom}%</button><button aria-label="Zoom in" disabled={blocked} onClick={() => setZoom((value) => Math.min(200, value + 10))}><Plus size={15}/></button></div>
      <button className="icon-button" aria-label="Find" title="Find (Ctrl+F)" disabled={blocked} onClick={openSearch}><Search size={18}/></button>
      <button className="icon-button" aria-label="Print" title="Print (Ctrl+P)" disabled={blocked||adapter.kind==="demo"} onClick={openPrint}><Printer size={18}/></button>
      <SaveCopyButton key={activeTab!.id} disabled={blocked} demo={adapter.kind === "demo"} onSave={flatten=>void exportPdf(flatten)}/>
    </div>
    <div className="workspace" role="tabpanel" aria-labelledby={"tab-" + activeTab!.id}>
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
        {recovery.warning&&<div className="recovery-warning" role="status">{recovery.warning}</div>}
        {searchState.open&&<SearchBar key={"search-"+activeTab!.id} query={searchState.query} onQueryChange={query=>updateSearch({query,index:0})} index={searchState.index} total={search.matches.length} searching={search.searching} error={search.error} hasText={search.hasText} onNext={()=>moveSearch(1)} onPrevious={()=>moveSearch(-1)} onClose={()=>updateSearch({open:false})}/>}
        {tool === "edit" && <div className="placement-banner"><TextCursorInput size={16}/> Click an outlined text run to edit it. Some fonts and layouts are not supported yet.</div>}
        {tool === "highlight" && <div className="placement-banner"><Highlighter size={16}/> Drag across selectable text to highlight it. Scanned pages need a text layer.</div>}
        {tool === "comment" && <div className="placement-banner"><MessageSquare size={16}/> Click a page to add a comment.</div>}
        {pendingSignature && <div className="placement-banner"><PenLine size={16}/> Click a page to place your signature <button aria-label="Cancel signature placement" onClick={() => { setPendingSignature(null); setTool("select"); }}><X size={15}/></button></div>}
        <DocumentViewport key={activeTab!.id} initialScrollPosition={scrollPositions.current.get(activeTab!.id)} onScrollPositionChange={position => { scrollPositions.current.set(activeTab!.id, position); recovery.schedule(); }} adapter={adapter} pages={current.pages} selectedPageId={selectedPage?.id ?? null} selectedOverlayId={current.selectedOverlayId}
          zoom={zoom} onZoomChange={setZoom} viewMode={preferences.viewMode} tool={tool} penColor={preferences.penColor} penWidth={preferences.penWidth}
          pendingSignature={pendingSignature} interactionDisabled={blocked} navigationRequest={navigationRequest} searchMatches={searchState.open?search.matches:[]} activeSearchMatchId={searchMatch?.id}
          onSelectPage={selectPage} onEditText={beginTextEdit} onSelectOverlay={selectOverlay} onAddText={addText} onPlaceSignature={placeSignature} onMoveOverlay={moveOverlay} onDraw={addDrawing} onHighlight={addHighlight} onAddComment={addComment}/>
        <div className="page-nav"><button aria-label="Previous page" disabled={blocked || selectedIndex <= 0} onClick={() => navigateToPage(current.pages[selectedIndex - 1].id)}><ChevronLeft size={16}/></button><span>Page {selectedIndex + 1} of {current.pages.length}</span><button aria-label="Next page" disabled={blocked || selectedIndex >= current.pages.length - 1} onClick={() => navigateToPage(current.pages[selectedIndex + 1].id)}><ChevronRight size={16}/></button></div>
      </div>
      <aside className="properties">
        <div className="panel-heading"><span>PROPERTIES</span></div>
        {tool === "draw" ? <section className="property-section"><h3>Draw</h3>
          <label className="field-label">Pen color<span className="color-input"><input aria-label="Pen color" type="color" value={preferences.penColor} onChange={(event) => setPreferences({ ...preferences, penColor: event.target.value.toUpperCase() })}/><code>{preferences.penColor}</code></span></label>
          <label className="field-label">Pen width · {preferences.penWidth} pt<input aria-label="Pen width" type="range" min="0.5" max="20" step="0.5" value={preferences.penWidth} onChange={(event) => setPreferences({ ...preferences, penWidth: Number(event.target.value) })}/></label>
          <p>Draw directly on any page. Each stroke can be undone. Switch to Select to move or delete a stroke.</p>
        </section> : selectedOverlay && selectedPage ? <OverlayProperties pageWidth={selectedPage.width} pageHeight={selectedPage.height} autoEdit={selectedOverlay.id === newTextId} onAutoEdited={() => setNewTextId(null)} overlay={selectedOverlay} onChange={(update) => edit((document) => updateOverlay(document, selectedPage.id, selectedOverlay.id, update))} onDelete={() => edit((document) => removeOverlay(document, selectedPage.id, selectedOverlay.id))}/> : selectedPage && <>
          <section className="property-section"><h3>Page</h3><div className="page-summary"><div className="mini-page" style={{ aspectRatio: `${displaySize!.width}/${displaySize!.height}` }}/><div><strong>Page {selectedIndex + 1}</strong><span>{Math.round(selectedPage.width)} × {Math.round(selectedPage.height)} pt</span><small>{selectedPage.rotation ? `${selectedPage.rotation}° clockwise` : "Original orientation"}</small></div></div></section>
          <section className="property-section"><h3>Arrange</h3><div className="property-grid"><button disabled={blocked || selectedIndex <= 0} onClick={() => edit((document) => movePage(document, selectedPage.id, selectedIndex - 1))}><ArrowUp size={16}/>Move up</button><button disabled={blocked || selectedIndex >= current.pages.length - 1} onClick={() => edit((document) => movePage(document, selectedPage.id, selectedIndex + 1))}><ArrowDown size={16}/>Move down</button><button disabled={blocked} onClick={() => edit((document) => rotatePage(document, selectedPage.id))}><RotateCw size={16}/>Rotate</button><button disabled={blocked} onClick={() => { const id = uniqueId("page"); edit((document) => duplicatePage(document, selectedPage.id, id)); setNavigationRequest((request) => ({ pageId: id, revision: (request?.revision ?? 0) + 1 })); }}><Copy size={16}/>Duplicate</button></div></section>
          <section className="property-section"><h3>Page actions</h3><button className="danger-action" disabled={blocked || current.pages.length <= 1} onClick={() => edit((document) => deletePage(document, selectedPage.id))}><Trash2 size={16}/>Delete page</button><p>Source files are never changed.</p></section>
        </>}
        <ReviewList pages={current.pages} selectedPageId={selectedPage?.id??null} selectedOverlayId={current.selectedOverlayId} disabled={blocked} onSelect={navigateToAnnotation}/>
      </aside>
    </div>
    <footer className="statusbar"><span>{busy ?? notice ?? (dirty ? "Unsaved edits" : "All changes exported")}</span>{failure && <span className="status-error" role="alert">{failure}</span>}<span className="status-spacer"/><span>{selectedPage ? `${Math.round(selectedPage.width)} × ${Math.round(selectedPage.height)} pt` : ""}</span><span>Local only</span></footer>
    {dialogs}
  </div>;
}
