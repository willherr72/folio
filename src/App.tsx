import {ParagraphViewportHost} from "./components/ParagraphViewportHost";
import {TextRemovalDialog} from './components/TextRemovalDialog';
import {captureTextRemoval,commitTextRemoval,supportsTextRemoval,supportsContentRemoval,type RemovalRequest} from './editor/text-removal-workflow';
import type {CanvasParagraphView} from "./editor/paragraph-canvas";
import type {CanvasImageView} from "./editor/existing-image-canvas";
import {ImageReplacementDialog} from "./components/ImageReplacementDialog";
import type {ExistingImage} from "./editor/types";
import {paragraphEditorKind,supportsOccurrenceParagraph,type ParagraphOccurrence} from "./editor/paragraph-transaction";
import {prepareFormParagraph,supportsFormParagraph} from "./editor/form-paragraph";
import {prepareParagraphDraft,type ExistingTextDraft,type PreparedParagraphDraft} from "./editor/paragraph-draft";
import {NativeCommentDialog} from "./components/NativeCommentDialog";
import {useNativeComments,commitNativeCommentReplacement} from "./editor/native-comments";
import type {NativeComment,NativeCommentChanges} from "./editor/types";
import {useWorkspaceImages} from "./editor/use-image-resources";
import {useWorkspaceSources} from "./editor/use-workspace-sources";
import {useActiveTabVisibility} from "./editor/use-active-tab-visibility";
import {PropertiesPanel} from "./components/PropertiesPanel";
import {useImagePlacement} from "./editor/use-image-placement";
import {drainImageAcquisitions,flushImageReleases} from "./editor/image-resources";
import {moveImage,imageBounds} from "./editor/image-geometry";
import {discardOpenedDocument} from "./editor/discard-opened-document";
import {canTravelNavigation,recordNavigation,travelNavigation,type NavigationDirection,type NavigationRequest,type ReadingLocation} from './editor/navigation-history';
import {PasswordDialog} from "./components/PasswordDialog";
import {usePasswordWorkflow} from "./editor/usePasswordWorkflow";
import {commitOwnerUnlock,pageAllows,canArrangePages,canFlattenPages,toolAllowed,allowsWorkspaceEdit,describeRestrictions} from "./editor/protection";
import {BlankPageDialog,type BlankPageOptions,type BlankPagePosition} from "./components/BlankPageDialog";
import {ImagePagesDialog} from "./components/ImagePagesDialog";
import {BookmarkSidebar} from "./components/BookmarkSidebar";
import {BookmarkDialog} from "./components/BookmarkDialog";
import {PageJumpDialog} from "./components/PageJumpDialog";
import {addBookmark,editBookmark,removeBookmark,moveBookmark,importBookmarks,filterBookmarks,MAX_BOOKMARKS} from "./editor/bookmarks";
import {SplitPdfDialog} from "./components/SplitPdfDialog";
import {PageOrganizerDialog} from "./components/PageOrganizerDialog";
import {applyOrganizerDocument} from "./editor/page-organizer";
import {ExportImagesDialog} from "./components/ExportImagesDialog";
import {ExtractPagesDialog} from "./components/ExtractPagesDialog";
import {CropPageDialog} from "./components/CropPageDialog";
import {FormAuthorDialog} from "./components/FormAuthorDialog";
import {FormPositionDialog} from "./components/FormPositionDialog";
import {FormTextAppearanceDialog} from "./components/FormTextAppearanceDialog";
import {FormFrameDialog} from "./components/FormFrameDialog";
import {formAuthorSelection} from "./editor/form-authoring";
import {PageReplacementDialog} from "./components/PageReplacementDialog";
import {commitPageReplacement,pageReplacementRequest} from "./editor/page-replacement";
import {cropPage} from "./editor/page-crop";
import {TextParagraphDialog} from "./components/TextParagraphDialog";
import {StyledParagraphDialog} from "./components/StyledParagraphDialog";
import { FontPicker } from "./components/FontPicker";
import { drainFontAcquisitions, flushFontReleases, retainCustomFonts, holdCustomFont, type FontInfo } from "./editor/custom-fonts";
import { useWorkspaceFonts, workspaceFontIds } from "./editor/use-font-resources";
import { useCallback, useMemo, useEffect, useLayoutEffect, useRef, useState, type SetStateAction } from "react";
import {
  ArrowDown, ArrowUp, ArrowLeft, ArrowRight, ChevronLeft, ChevronRight, Copy, FilePlus2, FolderOpen,
   Highlighter, MessageSquare, Search, Printer, Minus, MousePointer2, PenLine, Pencil, Plus, Redo2, RotateCw, Settings, Square, Trash2, Type, TextCursorInput, Undo2, X,
} from "lucide-react";
import { createDemoAdapter, documentToPages, nativeAdapter, type FolioAdapter } from "./editor/adapter";
import { pageVisibleRect, placeInkPaths, fromDisplayPoint } from "./editor/geometry";
import { defaultPageTextArea, moveArea } from "./editor/text-area-geometry";
import {
  commit, MAX_DOCUMENT_HISTORY_ACTIONS, deletePage, duplicatePage, insertPages, movePage, planDigest, redo, removeOverlay,
  rotatePage, undo, uniqueId, updateOverlay, type EditorDocument, type History, type Overlay,
} from "./editor/model";
import { addSession, createSession, emptyWorkspace, removeSession, updateSession, type DocumentSession, type ScrollPosition } from "./editor/workspace";
import type { AnnotationRect, EditorTool, InkPoint, TextOverlay, DocumentInfo, ImagePageOptions, DocumentBookmark, PageContentReplacement } from "./editor/types";
import { usePreferences } from "./editor/preferences";
import { clearRenderCache } from "./components/PageView";
import {PageThumbnails} from "./components/PageThumbnails";
import { DocumentViewport } from "./components/DocumentViewport";
import { SignaturePad } from "./components/SignaturePad";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { SettingsDialog } from "./components/SettingsDialog";
import {useWorkspaceRecovery} from "./editor/useWorkspaceRecovery";
import {searchNavigationRect,useDocumentSearch} from "./editor/search";
import {SearchBar} from "./components/SearchBar";
import {PrintDialog} from "./components/PrintDialog";
import {RecoveryDialog} from "./components/RecoveryDialog";
import {printPdf,type PrintOptions} from "./editor/printing";
import type {PagePlan} from "./editor/types";
import "./styles.css";
import {ExportTextDialog} from "./components/ExportTextDialog";
import {ProtectedCopyDialog} from './components/ProtectedCopyDialog';
import {PdfSaveProgressDialog,type PdfSaveView} from './components/PdfSaveProgressDialog';
import type {PdfSaveTask} from './editor/pdf-save';
import {SaveCopyButton} from "./components/SaveCopyButton";
import { OverlayProperties } from "./components/OverlayProperties";
import { ExistingTextDialog } from "./components/ExistingTextDialog";
import { TextGroupDialog } from "./components/TextGroupDialog";
import { commitTextReplacement,supportsOccurrenceGroup,supportsTextGroup,supportsOccurrenceForm,supportsFormText } from "./editor/text-edit";
import {commitFormReplacement} from "./editor/form-filling";
import {FormFillDialog} from "./components/FormFillDialog";
import {FormFillEditor} from "./components/FormFillEditor";
import {FormPropertiesDialog} from "./components/FormPropertiesDialog";
import {DocumentPropertiesDialog} from "./components/DocumentPropertiesDialog";
import {validateMetadata} from "./editor/document-properties";
import {FormRemovalDialog} from './components/FormRemovalDialog';
import {applyFormRemoval,type RemoveFormFieldRequest} from './editor/form-removal';
import {FormChoicesDialog} from "./components/FormChoicesDialog";
import {applyFormProperties,type SetFormPropertiesRequest} from './editor/form-properties';
import {formCanvasTargetBounds} from "./components/FormPageTargets";
import {updateFormAnchor,type FormCanvasAnchor,type FormAnchorCallback} from './editor/form-canvas-entry';
import {OcrDialog} from "./components/OcrDialog";
import {commitOcrReplacements} from "./editor/ocr/workflow";
import type {SourceReplacement} from "./editor/types";
import type {FormField,FormFieldValue} from "./editor/types";
import type {EditableTextRun,TextGroupPreview} from "./editor/types";
import { ReviewList } from "./components/ReviewList";

interface AppProps { initialDemo?: boolean }
type Tool = EditorTool;
type Confirmation = { title: string; description: string; confirmLabel: string };
function errorMessage(error: unknown) { return error instanceof Error ? error.message : String(error); }

export function App({ initialDemo = new URLSearchParams(location.search).get("demo") === "1" }: AppProps) {
  const { preferences, setPreferences, resetPreferences } = usePreferences();
  const [workspace, setWorkspace] = useState(emptyWorkspace);
  const tabStripRef = useActiveTabVisibility(workspace.activeId, workspace.tabs.length);
  useWorkspaceFonts(workspace);
  useWorkspaceImages(workspace);
  const workspaceCurrent = useRef(workspace); workspaceCurrent.current = workspace;
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const activeTab = workspace.tabs.find(tab => tab.id === workspace.activeId) ?? null;
  const history = activeTab?.history ?? null;
  const savedDigest = activeTab?.savedDigest ?? "";
  const adapter = activeTab?.adapter ?? nativeAdapter;
  const zoom = activeTab?.zoom ?? preferences.defaultZoom;
  const navigationRequest = activeTab?.navigationRequest ?? null;
  const captureViewRef=useRef<(()=>ReadingLocation|null)|null>(null);
  const scrollPositions = useRef(new Map<string, ScrollPosition>());
  const isDesktop = "__TAURI_INTERNALS__" in window;
  const recovery = useWorkspaceRecovery(isDesktop,workspace,setWorkspace,scrollPositions);
  useWorkspaceSources(workspace,setWorkspace,recovery.sourceOwnership);
  const recoveryCurrent = useRef(recovery); recoveryCurrent.current = recovery;
  const [blankRequest,setBlankRequest] = useState<{tabId:string|null;pageId:string|null}|null>(null);
  const [imageRequest,setImageRequest] = useState<{tabId:string|null;pageId:string|null}|null>(null);
  const [extraction,setExtraction] = useState<{pages:PagePlan[];currentPageId:string|null;adapter:FolioAdapter;bookmarks?:DocumentBookmark[];bookmarkError?:string;metadata?:EditorDocument["metadata"]}|null>(null);
  const [navigationView,setNavigationView]=useState<"pages"|"bookmarks">("pages");
  const [bookmarkDialog,setBookmarkDialog]=useState<{id?:string;parentId?:string}|null>(null);
  const [pageJump,setPageJump]=useState(false);
  const [printOpen,setPrintOpen] = useState(false);
  const [searchTabs,setSearchTabs] = useState<Record<string,{open:boolean;query:string;index:number}>>({});
  const followedSearch=useRef(new Map<string,string>());
  const searchState = searchTabs[workspace.activeId??""]??{open:false,query:"",index:0};
  const search = useDocumentSearch(adapter,history?.present.pages??[],searchState.query,searchState.open);
  const searchMatch = search.matches[Math.min(searchState.index,search.matches.length-1)];
  const searchPage=history?.present.pages.find(page=>page.id===searchMatch?.pageId);
  const searchTarget=searchMatch&&searchPage?searchNavigationRect(searchPage,searchMatch):undefined;
  const searchNavigationKey=searchMatch&&searchTarget?JSON.stringify([searchState.query,searchMatch.id,searchTarget]):null;
  const updateSearch = useCallback((patch:Partial<typeof searchState>)=>{
    if(!workspace.activeId)return;
    // A deliberate query edit may revisit the same result; tab/modal returns may not.
    if(patch.query!==undefined)followedSearch.current.delete(workspace.activeId);
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
  const [textEditMode,setTextEditMode]=useState<"text"|"paragraph">("text");
  const [imageAspectLocked,setImageAspectLocked]=useState(true);
  const [pendingSignature, setPendingSignature] = useState<InkPoint[][] | null>(null);
  const [fontPicker, setFontPicker] = useState<{tabId: string; pageId: string; overlayId: string; text: string; fontId?: string; fontSize: number; shaping?: TextOverlay["shaping"]; area?: TextOverlay["area"]; createAt?: InkPoint} | null>(null);
  const [textEdit,setTextEdit] = useState<{tabId:string;document:DocumentSession["history"]["present"];page:PagePlan;run:EditableTextRun;adapter:FolioAdapter}|null>(null);
  const [textParagraph,setTextParagraph] = useState<(NonNullable<typeof textEdit>&{initialDraft?:PreparedParagraphDraft})|null>(null);
  const paragraphDraftLease=useRef<(()=>Promise<void>)|null>(null);
  useEffect(()=>{if(!textParagraph){const done=paragraphDraftLease.current;paragraphDraftLease.current=null;void done?.();}},[textParagraph]);
  useEffect(()=>()=>{const done=paragraphDraftLease.current;paragraphDraftLease.current=null;void done?.();},[]);
  const [paragraphModal,setParagraphModal]=useState(false);
  const paragraphToolButton=useRef<HTMLButtonElement>(null),directParagraphToolButton=useRef<HTMLButtonElement>(null),priorParagraphSession=useRef(false);
  useEffect(()=>{if(priorParagraphSession.current&&!textParagraph){setParagraphModal(false);(textEditMode==="paragraph"?directParagraphToolButton:paragraphToolButton).current?.focus({preventScroll:true});}priorParagraphSession.current=!!textParagraph;},[textParagraph,textEditMode]);
  const paragraphOccurrence=useMemo<ParagraphOccurrence|undefined>(()=>textParagraph?{
    document:textParagraph.document,page:textParagraph.page,
    isSourceRetained:id=>workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(id)),
  }:undefined,[textParagraph]);
  const [textEditError,setTextEditError] = useState<string|null>(null);
  const [nativeComment,setNativeComment]=useState<{tabId:string;page:PagePlan;document:EditorDocument;adapter:FolioAdapter;comment:NativeComment}|null>(null);
  const [formFill,setFormFill]=useState<{tabId:string;page:PagePlan;document:EditorDocument;adapter:FolioAdapter}|null>(null);
  const [formOverview,setFormOverview]=useState(false),[formDraftDirty,setFormDraftDirty]=useState(false);
  const [formTarget,setFormTarget]=useState<{annotationIndex?:number;revision:number}>({revision:0});
  const [activeFormField,setActiveFormField]=useState<FormField|null>(null);
  const [formAnchor,setFormAnchor]=useState<FormCanvasAnchor|null>(null);
  const receiveFormAnchor=useCallback<FormAnchorCallback>((identity,element,previous)=>setFormAnchor(current=>updateFormAnchor(current,identity,element,previous)),[]);
  const formDirtyRef=useRef(false);
  const formFillButton=useRef<HTMLButtonElement>(null),formWasOpen=useRef(false);
  useLayoutEffect(()=>{
    if(formWasOpen.current&&!formFill){
      const target=formFillButton.current;
      if(target&&!target.disabled)target.focus();
      else document.querySelector<HTMLButtonElement>('button[aria-label="Zoom in"]')?.focus();
    }
    formWasOpen.current=!!formFill;
  },[formFill]);
  const [cropRequest,setCropRequest]=useState<typeof formFill>(null);
  const [formPosition,setFormPosition]=useState<(NonNullable<typeof formFill>&{sourceIds:string[];field:FormField})|null>(null);
  const [formTextAppearance,setFormTextAppearance]=useState<typeof formPosition>(null);
  const [formFrame,setFormFrame]=useState<typeof formPosition>(null);
  const [formProperties,setFormProperties]=useState<typeof formPosition>(null);
  const [documentProperties,setDocumentProperties]=useState<{tabId:string;document:EditorDocument;adapter:FolioAdapter}|null>(null);
  const [formRemoval,setFormRemoval]=useState<typeof formPosition>(null);
  const [formChoices,setFormChoices]=useState<typeof formPosition>(null);
  const [formAuthor,setFormAuthor]=useState<(NonNullable<typeof formFill>&{sourceIds:string[]})|null>(null);
  const [pageReplacement,setPageReplacement]=useState<(NonNullable<typeof formFill>&{sourceIds:string[]})|null>(null);
  const [textRemoval,setTextRemoval]=useState<RemovalRequest|null>(null);
  const [imageReplacement,setImageReplacement]=useState<(NonNullable<typeof pageReplacement>&{image:ExistingImage})|null>(null);
  const [textExport,setTextExport]=useState<{pages:PagePlan[];currentPageId:string|null}|null>(null);
  const [pdfSave,setPdfSave]=useState<PdfSaveView|null>(null);
  const pdfSaveTask=useRef<PdfSaveTask|null>(null),pdfSaveCancelPending=useRef(false);
  useEffect(()=>()=>{void pdfSaveTask.current?.cancel().catch(()=>{});},[]);
  const [pdfSplit,setPdfSplit]=useState<{pages:PagePlan[];bookmarks?:EditorDocument["bookmarks"];bookmarkError?:string;metadata?:EditorDocument["metadata"]}|null>(null);
  const [organizer,setOrganizer]=useState<{tabId:string;document:EditorDocument;adapter:FolioAdapter}|null>(null);
  const [securityCopy,setSecurityCopy]=useState<{pages:PagePlan[];bookmarks?:DocumentBookmark[];metadata?:EditorDocument["metadata"]}|null>(null);
  const [imageExport,setImageExport]=useState<{pages:PagePlan[];currentPageId:string|null}|null>(null);
  const [ocr,setOcr]=useState<{tabId:string;document:EditorDocument;adapter:FolioAdapter}|null>(null);
  const [textGroup,setTextGroup] = useState<{tabId:string;document:EditorDocument;page:PagePlan;run:EditableTextRun;adapter:FolioAdapter;runs:EditableTextRun[];loading:boolean}|null>(null);
  const [textGroupError,setTextGroupError] = useState<string|null>(null);
  const groupGeneration=useRef(0);
  const [signatureOpen, setSignatureOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const passwords=usePasswordWorkflow();
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
  const canArrange=canArrangePages(current?.pages??[]);
  const canAnnotate=pageAllows(selectedPage,"annotate");
  const canPrint=!!current?.pages.some(page=>pageAllows(page,"print"));
  const canFlatten=canFlattenPages(current?.pages??[]);
  let canReplacePage=false;
  if(current&&selectedPage&&adapter.replacePageContent&&adapter.renderPagePlan){try{pageReplacementRequest(current,selectedPage);canReplacePage=true;}catch{/* Native policy is also checked during preparation. */}}
  useEffect(()=>{if(!textParagraph&&!imageReplacement&&!toolAllowed(selectedPage,tool)){setTool("select");setPendingSignature(null);}},[selectedPage,tool,imageReplacement,textParagraph]);
  const priorImageSession=useRef(false),imageToolButton=useRef<HTMLButtonElement>(null);
  useEffect(()=>{
    if(priorImageSession.current&&!imageReplacement)imageToolButton.current?.focus({preventScroll:true});
    priorImageSession.current=!!imageReplacement;
  },[imageReplacement]);
  const modalOpen = !!documentProperties || !!textRemoval || !!formFrame || !!formRemoval || !!formChoices || !!formProperties || !!formTextAppearance || !!formPosition || !!formAuthor || !!securityCopy || !!pdfSave || !!organizer || !!pdfSplit || !!textExport || !!pageReplacement || !!imageExport || !!cropRequest || !!nativeComment || !!passwords.prompt || !!ocr || (!!formFill&&formOverview) || !!fontPicker || !!textEdit || paragraphModal || !!textGroup || signatureOpen || settingsOpen || printOpen || !!extraction || !!blankRequest || !!imageRequest || !!bookmarkDialog || pageJump || !!confirmation || !recovery.ready;
  const imagePlacement=useImagePlacement({workspace,setWorkspace,disabled:modalOpen||!!busy,onPlaced:()=>{setTool("select");setNotice("Image placed. Save a copy when finished.");}});
  const nativeReview=useNativeComments(adapter,current?.pages??[]);
  const readingBlocked = modalOpen || !!busy || imagePlacement.importing;
  const blocked = readingBlocked || !!formFill || !!imageReplacement || !!textParagraph;
  const captureReadingLocation=useCallback(()=>captureViewRef.current?.()??(selectedPage?{pageId:selectedPage.id,zoom}:null),[selectedPage,zoom]);
  const setReadingZoom=useCallback((action:SetStateAction<number>)=>{
    const origin=captureViewRef.current?.();
    setWorkspace(value=>updateSession(value,workspace.activeId,tab=>{
      const zoom=typeof action==='function'?action(tab.zoom):action;
      if(zoom===tab.zoom)return tab;
      return {...tab,zoom,...(origin?{
        history:{...tab.history,present:{...tab.history.present,selectedPageId:origin.pageId}},
        navigationRequest:{pageId:origin.pageId,point:origin.point,revision:(tab.navigationRequest?.revision??0)+1},
      }:{})};
    }));
  },[workspace.activeId]);
  const navigateReading=useCallback((pageId:string,rect?:NavigationRequest['rect'],overlayId:string|null=null)=>{
    const origin=captureReadingLocation();
    setWorkspace(value=>updateSession(value,workspace.activeId,tab=>{
      if(!tab.history.present.pages.some(page=>page.id===pageId))return tab;
      return {...tab,history:{...tab.history,present:{...tab.history.present,selectedPageId:pageId,selectedOverlayId:overlayId}},
        navigationRequest:{pageId,rect,revision:(tab.navigationRequest?.revision??0)+1,...(origin?{origin}:{})}};
    }));
  },[captureReadingLocation,workspace.activeId]);
  const completeNavigation=useCallback((request:NavigationRequest,destination:ReadingLocation|null)=>{
    if(!request.origin||!destination)return;
    setWorkspace(value=>updateSession(value,workspace.activeId,tab=>{
      if(tab.navigationRequest?.revision!==request.revision||tab.navigationRequest.pageId!==request.pageId)return tab;
      const navigationHistory=recordNavigation(tab.navigationHistory,request.origin!,destination);
      return navigationHistory===tab.navigationHistory?tab:{...tab,navigationHistory};
    }));
  },[workspace.activeId]);
  const travelReading=useCallback((direction:NavigationDirection)=>{
    if(readingBlocked)return;
    const origin=captureReadingLocation();if(!origin)return;
    const live=workspaceCurrent.current.tabs.find(tab=>tab.id===workspace.activeId);
    if(!live||!travelNavigation(live.navigationHistory,origin,direction,live.history.present.pages.map(page=>page.id)))return;
    updateSearch({open:false});if(!imageReplacement&&!textParagraph)setTool('select');setPendingSignature(null);
    setWorkspace(value=>updateSession(value,workspace.activeId,tab=>{
      const result=travelNavigation(tab.navigationHistory,origin,direction,tab.history.present.pages.map(page=>page.id));
      if(!result)return tab;
      return {...tab,navigationHistory:result.history,zoom:result.target.zoom,
        history:{...tab.history,present:{...tab.history.present,selectedPageId:result.target.pageId,selectedOverlayId:null}},
        navigationRequest:{pageId:result.target.pageId,point:result.target.point,revision:(tab.navigationRequest?.revision??0)+1}};
    }));
  },[textParagraph,imageReplacement,readingBlocked,captureReadingLocation,updateSearch,workspace.activeId]);
  useEffect(()=>{if(tool==="image"&&!imagePlacement.pending&&!imagePlacement.importing)setTool("select");},[tool,imagePlacement.pending,imagePlacement.importing]);
  useLayoutEffect(()=>{if(tool!=="image"&&imagePlacement.pending)imagePlacement.cancel();},[tool,imagePlacement.pending,imagePlacement.cancel]);
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
    const tabId=workspace.activeId;if(!tabId)return;
    if(!searchState.open){followedSearch.current.delete(tabId);return;}
    if(!blocked&&searchMatch&&searchNavigationKey&&followedSearch.current.get(tabId)!==searchNavigationKey){
      followedSearch.current.set(tabId,searchNavigationKey);
      navigateReading(searchMatch.pageId,searchTarget);
    }
  },[blocked,searchState.open,searchNavigationKey,workspace.activeId,navigateReading]);
  const openSearch=useCallback(()=>{
    if(blocked||!current)return;
    updateSearch({open:true});setTool("select");setPendingSignature(null);
    document.querySelector<HTMLInputElement>('[aria-label="Find in document"]')?.select();
  },[blocked,current,updateSearch]);
  const moveSearch=(direction:number)=>{if(search.matches.length)updateSearch({index:(Math.min(searchState.index,search.matches.length-1)+direction+search.matches.length)%search.matches.length});};
  const openPrint=useCallback(()=>{
    if(blocked||!current||!canPrint)return;
    if(adapter.kind!=="native"||!isDesktop){setFailure("Printing is available for PDFs in the Windows desktop app.");return;}
    setPrintOpen(true);
  },[blocked,current,adapter.kind,isDesktop,canPrint]);
  const startPrint=async(pages:PagePlan[],options:PrintOptions)=>{
    if(operation.current)return;
    operation.current=true;setPrintOpen(false);setBusy("Preparing print job…");setFailure(null);
    try{const printed=await printPdf(pages,options);setNotice(printed?"Document sent to printer":"Printing cancelled");}
    catch(error){setFailure("Couldn’t print: "+errorMessage(error));}
    finally{operation.current=false;setBusy(null);}
  };
  const releaseSession = useCallback(async (tab: DocumentSession) => {
    await drainImageAcquisitions();
    clearRenderCache(tab.sourceIds);
    await Promise.allSettled(tab.sourceIds.map(sourceId => tab.adapter.closeDocument(sourceId)));
    scrollPositions.current.delete(tab.id);
    followedSearch.current.delete(tab.id);
  }, []);
  const clearTransient = useCallback(() => {
    imagePlacement.cancel();
    setImageReplacement(null);
    setOcr(null);setFormFill(null);setFormOverview(false);setFormDraftDirty(false);formDirtyRef.current=false;
    setBookmarkDialog(null);setPageJump(false);
    setTextParagraph(null);groupGeneration.current++;setTextGroup(null);setTextGroupError(null);
    setTool("select"); setPendingSignature(null); setNewTextId(null); setTextEdit(null); setTextEditError(null); setFontPicker(null);
    setNotice(null); setFailure(null); setDraggedPageId(null); setDropTarget(null);
    window.getSelection()?.removeAllRanges();
  }, [imagePlacement.cancel]);
  const switchTab = useCallback((id: string) => {
    if (operation.current || blocked || id === workspace.activeId) return;
    const start = dragOverlay.current;
    if (start) {
      setHistory(value => value && planDigest(value.present) !== planDigest(start.document)
        ? commit({...value,present:start.document},()=>value.present) : value);
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
    updateSearch({open:false});navigateReading(pageId);
  }, [navigateReading,updateSearch]);
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
      if(!canArrangePages(activeTab.history.present.pages)||info.protection?.permissions.assemble===false){throw new Error("This PDF does not allow combining pages. Open it in its own tab.");}
      const newPages = documentToPages(info);
      const incoming=importBookmarks(info.bookmarks,newPages);
      const bookmarks=activeTab.history.present.bookmarks!==undefined||incoming.length?[...(activeTab.history.present.bookmarks??[]),...incoming]:undefined;
      if(bookmarks&&bookmarks.length>MAX_BOOKMARKS){throw new Error("Adding this PDF would exceed the 4000-bookmark limit.");}
      setWorkspace(value => updateSession(value, activeTab.id, tab => ({
        ...tab, sourceIds:[...new Set([...tab.sourceIds,info.id])],
        history:commit(tab.history, document => ({...document,pages:[...document.pages,...newPages],selectedPageId:newPages[0]?.id??document.selectedPageId,selectedOverlayId:null,
          ...(bookmarks!==undefined?{bookmarks}:{}),...(document.bookmarkError||info.bookmarkError?{bookmarkError:document.bookmarkError??info.bookmarkError}:{})})),
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
      await drainFontAcquisitions(); await flushFontReleases(); await drainImageAcquisitions(); await flushImageReleases(); installDocument(await passwords.open(nativeAdapter), nativeAdapter);
    } catch (error) { setFailure("Couldn’t open PDF: " + errorMessage(error)); }
    finally { operation.current = false; setBusy(null); }
  }, [installDocument, modalOpen]);

  const addPdf = useCallback(async () => {
    if (operation.current || modalOpen || !current || !canArrange || adapter.kind !== "native") return;
    operation.current = true;
    setBusy("Adding PDF…"); setFailure(null);
    let incoming: Awaited<ReturnType<FolioAdapter["openPdf"]>> = null;
    try {
      await drainFontAcquisitions(); await flushFontReleases(); await drainImageAcquisitions(); await flushImageReleases();
      incoming = await passwords.open(nativeAdapter);
      if (incoming && (incoming.protection || current.pages.some(page=>page.protection))) {
        if (!nativeAdapter.checkSourceCombination) throw new Error("Protected PDF combination requires the desktop service.");
        await nativeAdapter.checkSourceCombination([...new Set([...current.pages.map(page=>page.sourceId),incoming.id])]);
      }
      const live=workspaceCurrent.current.tabs.find(tab=>tab.id===activeTab?.id);
      if(!mounted.current||!live)return;
      if(live.history.present.pages!==current.pages)throw new Error("The document changed while adding this PDF. Try again.");
      installDocument(incoming, nativeAdapter, true);
      incoming = null;
    }
    catch (error) { if(mounted.current)setFailure(`Couldn’t add PDF: ${errorMessage(error)}`); }
    finally {
      if (incoming) await discardOpenedDocument(incoming,id=>nativeAdapter.closeDocument(id)).catch(()=>{});
      operation.current = false; if(mounted.current)setBusy(null);
    }
  }, [adapter.kind, current, installDocument, modalOpen,canArrange,activeTab?.id]);

  const unlockEditing=async()=>{
    const page=selectedPage,tab=activeTab;
    if(blocked||operation.current||!page?.protection||page.protection.owner||!tab)return;
    const expected=tab.history.present;
    const ids=[...new Set(expected.pages.filter(item=>item.protection?.documentId===page.protection!.documentId).map(item=>item.sourceId))];
    operation.current=true;setFailure(null);setBusy("Unlocking editing…");
    let results:Awaited<ReturnType<typeof passwords.unlock>>=null;
    try {
      results=await passwords.unlock(tab.adapter,ids,expected.name);
      if(!results)return;
      const live=workspaceCurrent.current.tabs.find(item=>item.id===tab.id);
      if(!mounted.current||!live)throw new Error("The document was closed while unlocking.");
      const updated=commitOwnerUnlock(live,expected,results);
      setWorkspace(value=>updateSession(value,tab.id,()=>updated));
      results=null;setNotice("Editing unlocked. Saved copies keep the original protection.");
    } catch(error){if(mounted.current)setFailure("Couldn’t unlock editing: "+errorMessage(error));}
    finally {
      if(results)await Promise.allSettled(results.map(item=>tab.adapter.closeDocument(item.document.id)));
      operation.current=false;if(mounted.current)setBusy(null);
    }
  };

  const cancelPdfSave=async()=>{
    const task=pdfSaveTask.current;
    if(!task||pdfSaveCancelPending.current||pdfSave?.finishing||pdfSave?.cancelling)return;
    pdfSaveCancelPending.current=true;
    setPdfSave(value=>value?{...value,cancelling:true,error:null}:null);
    try{
      const accepted=await task.cancel();
      if(mounted.current&&pdfSaveTask.current===task&&!accepted)setPdfSave(value=>value?{...value,cancelling:false,finishing:true}:null);
    }catch(error){
      if(mounted.current&&pdfSaveTask.current===task)setPdfSave(value=>value?{...value,cancelling:false,error:`Could not request cancellation: ${errorMessage(error)}`}:null);
    }finally{if(pdfSaveTask.current===task)pdfSaveCancelPending.current=false;}
  };
  const exportPdf = useCallback(async (flatten = false) => {
    if (operation.current || modalOpen || !current?.pages.length || (flatten&&!canFlatten)) return;
    operation.current = true;
    setBusy(adapter.kind === "demo" ? "Preparing demo plan…" : "Exporting PDF…"); setFailure(null);setNotice(null);
    try {
      if(current.bookmarkError)throw new Error("Unreadable bookmarks must be discarded explicitly in the Bookmarks panel before saving.");
      const options={...(current.metadata!==undefined?{metadata:current.metadata}:{}),...(flatten?{flatten:true}:{}),...(current.bookmarks!==undefined?{bookmarks:filterBookmarks(current.bookmarks,current.pages)}:{})};
      let path:string|null;
      if(isDesktop&&adapter.startPdfSave){
        setPdfSave({progress:null,cancelling:false,finishing:false,error:null});pdfSaveCancelPending.current=false;
        const task=adapter.startPdfSave(current.pages,options,progress=>{
          if(mounted.current)setPdfSave(value=>value?{...value,progress,finishing:value.finishing||progress.stage==='finishing'}:null);
        });
        pdfSaveTask.current=task;path=await task.result;
      }else path=await (Object.keys(options).length?adapter.exportPdf(current.pages,options):adapter.exportPdf(current.pages));
      if(!mounted.current)return;
      if (path) { if (!flatten) setSavedDigest(planDigest(current)); setNotice(adapter.kind === "demo" ? "Demo edit plan downloaded" : flatten ? `Exported flattened text, images, and ink to ${path}` : `Saved a copy to ${path}`); }
      else if(isDesktop&&adapter.startPdfSave)setNotice('Save cancelled.');
    } catch (error) { if(mounted.current)setFailure(`Couldn’t export: ${errorMessage(error)}`); }
    finally { pdfSaveTask.current=null;pdfSaveCancelPending.current=false;operation.current = false;if(mounted.current){setBusy(null);setPdfSave(null);} }
  }, [adapter, current, modalOpen, setSavedDigest,canFlatten,isDesktop]);

  const openBlank = (insert:boolean) => {
    if(blocked || (insert && (!activeTab || !canArrange || adapter.kind!=="native")))return;
    if(!isDesktop){setFailure("Blank PDF creation is available in the Windows desktop app.");return;}
    setBlankRequest({tabId:insert?activeTab!.id:null,pageId:insert?selectedPage?.id??null:null});
  };
  const createBlank = async (options:BlankPageOptions) => {
    const request=blankRequest;
    if(!request || operation.current)return false;
    const sourceAdapter=request.tabId?workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId)?.adapter:nativeAdapter;
    if(!sourceAdapter?.createBlankDocument)throw new Error("Blank PDF creation is unavailable.");
    return createGenerated(request,sourceAdapter,()=>sourceAdapter.createBlankDocument!(options.width,options.height),options.position,"Blank page inserted");
  };
  const openImages = (insert:boolean) => {
    if(blocked || (insert && (!activeTab || !canArrange || adapter.kind!=="native")))return;
    if(!isDesktop){setFailure("Image import is available in the Windows desktop app.");return;}
    setImageRequest({tabId:insert?activeTab!.id:null,pageId:insert?selectedPage?.id??null:null});
  };
  const createImages = async(paths:string[],options:ImagePageOptions,position:BlankPagePosition) => {
    const request=imageRequest;
    if(!request || operation.current)return false;
    const sourceAdapter=request.tabId?workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId)?.adapter:nativeAdapter;
    if(!sourceAdapter?.createImageDocument)throw new Error("Image import is unavailable.");
    return createGenerated(request,sourceAdapter,()=>sourceAdapter.createImageDocument!(paths,options),position,`${paths.length} image pages inserted`);
  };
  const createGenerated = async(request:{tabId:string|null;pageId:string|null},sourceAdapter:FolioAdapter,create:()=>Promise<DocumentInfo>,position:BlankPagePosition,notice:string) => {
    operation.current=true;setBusy("Creating pages…");setFailure(null);
    let orphan:string|null=null;
    try {
      const info=await create();orphan=info.id;
      if(!mounted.current)return false;
      if(request.tabId){
        const tab=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
        if(!tab)throw new Error("The destination document has closed.");
        const pages=documentToPages(info);
        const anchor=tab.history.present.pages.findIndex(page=>page.id===request.pageId);
        if(anchor<0)throw new Error("The destination page has changed.");
        const index=position==="end"?tab.history.present.pages.length:anchor+(position==="after"?1:0);
        setWorkspace(value=>updateSession(value,request.tabId,session=>({...session,
          sourceIds:[...session.sourceIds,info.id],history:commit(session.history,document=>insertPages(document,pages,index)),
          navigationRequest:{pageId:pages[0].id,revision:(session.navigationRequest?.revision??0)+1},
        })));
        setNotice(notice);
        // Explicit page creation takes navigation priority over rebuilt Find hits.
        setSearchTabs(value=>({...value,[request.tabId!]:{...(value[request.tabId!]??{query:"",index:0}),open:false}}));
      } else {
        const session=createSession(info,sourceAdapter,preferences.defaultZoom);
        session.savedDigest="";
        setWorkspace(value=>addSession(value,session));clearTransient();
      }
      orphan=null;setTool("select");setPendingSignature(null);return true;
    } finally {
      if(orphan)void sourceAdapter.closeDocument(orphan).catch(()=>{});
      operation.current=false;if(mounted.current)setBusy(null);
    }
  };

  const openOrganizer = () => {
    if(blocked||!current||!canArrange||!activeTab)return;
    if(!current.pages.length||current.pages.length>10000){setFailure("Organizing supports documents with 1 to 10,000 pages.");return;}
    updateSearch({open:false});setOrganizer({tabId:activeTab.id,document:current,adapter});
  };
  const applyOrganization = (draft:EditorDocument) => {
    if(operation.current||!organizer)return;
    const request=organizer,tab=workspaceCurrent.current.tabs.find(t=>t.id===request.tabId);
    if(!tab)throw new Error("The document is no longer open.");
    const next=applyOrganizerDocument(tab,request.document,draft);
    setWorkspace(value=>updateSession(value,request.tabId,current=>current===tab?next:current));
    setOrganizer(null);setTool("select");setNotice("Page arrangement applied. Save a copy when finished.");
    if(draft.selectedPageId)setNavigationRequest(value=>({pageId:draft.selectedPageId!,revision:(value?.revision??0)+1}));
  };
  const extractOrganizedPages = async (draft:EditorDocument,pages:PagePlan[]) => {
    if(operation.current||!organizer)return null;
    const request=organizer;operation.current=true;
    try{
      if(draft.bookmarkError)throw new Error("Unreadable bookmarks must be resolved explicitly in the Bookmarks panel before extracting.");
      return await request.adapter.exportPdf(pages,{...(draft.bookmarks!==undefined?{bookmarks:filterBookmarks(draft.bookmarks,pages)}:{}),...(draft.metadata!==undefined?{metadata:draft.metadata}:{})});
    }finally{operation.current=false;}
  };
  const openSplit = () => {
    if(blocked||!current||!canArrange||adapter.kind!=="native")return;
    updateSearch({open:false});setPdfSplit({pages:current.pages,bookmarks:current.bookmarks,bookmarkError:current.bookmarkError,metadata:current.metadata});
  };
  const openExtract = () => {
    if(blocked || !current || !canArrange || adapter.kind!=="native")return;
    setExtraction({pages:current.pages,currentPageId:selectedPage?.id??null,adapter,bookmarks:current.bookmarks,bookmarkError:current.bookmarkError,metadata:current.metadata});
  };
  const extractPages = async (pages:PagePlan[]) => {
    if(operation.current || !extraction)return false;
    operation.current=true;setBusy("Extracting pages…");setFailure(null);
    try {
      if(extraction.bookmarkError)throw new Error("Unreadable bookmarks must be discarded explicitly in the Bookmarks panel before saving.");
      const path=await extraction.adapter.exportPdf(pages,{...(extraction.bookmarks!==undefined?{bookmarks:filterBookmarks(extraction.bookmarks,pages)}:{}),...(extraction.metadata!==undefined?{metadata:extraction.metadata}:{})});
      if(path)setNotice(`Extracted ${pages.length} page${pages.length===1?"":"s"} to ${path}`);
      return !!path;
    } finally {operation.current=false;setBusy(null);}
  };

  const edit = useCallback((update: (document: EditorDocument) => EditorDocument) => {
    if (blocked) return;
    setHistory((value) => value ? commit(value, document=>{const next=update(document);return allowsWorkspaceEdit(document,next)?next:document;}) : value);
  }, [blocked, setHistory]);
  const beginTextEdit = (pageId:string, run:EditableTextRun) => {
    if (blocked || !activeTab) return;
    const page = current?.pages.find(page=>page.id===pageId);
    if (!current||!page||!pageAllows(page,"editContent")) return;
    const request={tabId:activeTab.id,document:current,page,run,adapter};
    if(textEditMode==="paragraph"){void beginParagraphRequest(request);return;}
    if(!(run.objectPath ? supportsFormText(adapter) : ((adapter.listOccurrenceTextRuns&&adapter.replaceTextOccurrence)||adapter.replaceText)))return;
    setTextEdit(request);setTextEditError(null);
  };
  const applyTextEdit = async (replacement:string, fontId?: string) => {
    const request=textEdit;
    if (!request || operation.current) return;
    const editingAdapter=request.adapter;
    const formPath=request.run.objectPath;
    const completeSource=formPath?supportsOccurrenceForm(editingAdapter):!!editingAdapter.listOccurrenceTextRuns&&!!editingAdapter.replaceTextOccurrence;
    if (formPath ? (!request.run.supported || (!!fontId && !request.run.canSubstitute) || (!completeSource&&!editingAdapter.replaceFormText))
      : ((!request.run.supported && !(fontId && request.run.canSubstitute)) || (!completeSource&&!editingAdapter.replaceText))) return;
    operation.current=true;setBusy("Updating PDF text…");setTextEditError(null);
    let orphanSource:string|null=null;
    try {
      let occurrenceResult:PageContentReplacement|undefined;
      let result:DocumentInfo;
      if(completeSource){
        const target=pageReplacementRequest(request.document,request.page);
        if(formPath){
          const path=[...formPath];
          const args=[target.sourceId,target.occurrences,target.selectedOccurrence,path,request.run.text,replacement] as const;
          occurrenceResult=await (fontId?editingAdapter.replaceFormTextOccurrence!(...args,fontId):editingAdapter.replaceFormTextOccurrence!(...args));
        }else{
          const args=[target.sourceId,target.occurrences,target.selectedOccurrence,request.run.objectIndex,request.run.text,replacement] as const;
          occurrenceResult=await (fontId?editingAdapter.replaceTextOccurrence!(...args,fontId):editingAdapter.replaceTextOccurrence!(...args));
        }
        result=occurrenceResult.document;
      }else{
        const args = [request.page.sourceId,request.page.pageIndex,request.run.objectIndex,request.run.text,replacement] as const;
        result=await (formPath ? editingAdapter.replaceFormText!(request.page.sourceId,request.page.pageIndex,[...formPath],request.run.text,replacement,...(fontId?[fontId]:[]))
          : fontId ? editingAdapter.replaceText!(...args,fontId) : editingAdapter.replaceText!(...args));
      }
      const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
      const borrowed=workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(result.id));
      if(result.id&&!borrowed)orphanSource=result.id;
      if(!result.id||borrowed)throw new Error("The text edit must return a new source version.");
      if (!mounted.current || !session) return;
      // Validate before scheduling state; blocked workspace controls keep this version stable.
      const apply=(tab:DocumentSession)=>occurrenceResult?commitPageReplacement(tab,request.document,request.page,occurrenceResult):commitTextReplacement(tab,request.page,result);
      apply(session);
      setWorkspace(value=>updateSession(value,request.tabId,apply));
      orphanSource=null;setTextEdit(null);setTool("edit");setNotice("PDF text updated");
    } catch (error) {
      if (mounted.current) setTextEditError(errorMessage(error));
    } finally {
      if (orphanSource) await editingAdapter.closeDocument(orphanSource).catch(()=>{});
      operation.current=false;if (mounted.current) setBusy(null);
    }
  };
  const beginOcr=()=>{
    if(blocked||!activeTab||!current||!selectedPage||!pageAllows(selectedPage,"editContent")||!pageAllows(selectedPage,"copyContent")||!adapter.prepareOcrPage||!adapter.applyOcr)return;
    updateSearch({open:false});setOcr({tabId:activeTab.id,document:current,adapter});
  };
  const adoptOcr=(replacements:SourceReplacement[]):boolean=>{
    const request=ocr;
    if(!request||!mounted.current||operation.current||workspaceCurrent.current.activeId!==request.tabId)return false;
    const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
    if(!session||session.adapter!==request.adapter)return false;
    const next=commitOcrReplacements(session,request.document,replacements);
    setWorkspace(previous=>updateSession(previous,request.tabId,()=>next));
    setTool("select");setNotice("Searchable text applied. Save a copy when finished.");return true;
  };
  const openNativeComment=(pageId:string,comment:NativeComment)=>{
    if(blocked||!activeTab||!current)return;
    const page=current.pages.find(page=>page.id===pageId);if(!page)return;
    navigateReading(pageId,comment.bounds);updateSearch({open:false});setTool("select");
    setNativeComment({tabId:activeTab.id,page,document:current,adapter,comment});
  };
  const mutateNativeComment=async(changes:NativeCommentChanges|null,request=nativeComment):Promise<string|null>=>{
    if(!request||operation.current||(changes?!request.adapter.editNativeComment:!request.adapter.deleteNativeComment))throw new Error("Comment editing is unavailable.");
    operation.current=true;setBusy("Updating comment…");let orphan:string|null=null;
    try {
      const target={pageIndex:request.comment.pageIndex,annotationIndex:request.comment.annotationIndex,expectedText:request.comment.text};
      const result=changes?await request.adapter.editNativeComment!(request.page.sourceId,{...target,...changes}):await request.adapter.deleteNativeComment!(request.page.sourceId,target);
      if(result.id&&!workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(result.id)))orphan=result.id;
      const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
      if(!mounted.current||!session||session.adapter!==request.adapter)return null;
      const next=commitNativeCommentReplacement(session,request.document,request.page.sourceId,result);
      setWorkspace(previous=>updateSession(previous,request.tabId,()=>next));orphan=null;
      setNativeComment(null);setNotice(changes?"Comment updated. Save a copy when finished.":"Comment deleted. Use Undo to restore it.");
      return result.id;
    } finally {
      if(orphan)await request.adapter.closeDocument(orphan).catch(()=>{});
      operation.current=false;if(mounted.current)setBusy(null);
    }
  };
  const moveNativeComment=async(page:PagePlan,comment:NativeComment,position:InkPoint):Promise<string|null>=>{
    if(blocked||!activeTab||!current||operation.current||!pageAllows(page,"annotate")||comment.reason||!adapter.editNativeComment)throw new Error("This comment cannot be moved.");
    if(current.pages.find(value=>value.id===page.id)!==page||!nativeReview.notes.get(page.sourceId)?.includes(comment))throw new Error("The comment changed. Select it again and retry.");
    return mutateNativeComment({text:comment.text,position},{tabId:activeTab.id,page,document:current,adapter,comment});
  };
  const beginFormFill=()=>{
    if(blocked||!activeTab||!selectedPage||!pageAllows(selectedPage,"fillForms")||!current||!adapter.listFormFields||!adapter.fillFormField)return;
    updateSearch({open:false});
    imagePlacement.cancel();setPendingSignature(null);setTool("select");selectOverlay(selectedPage.id,null);
    formDirtyRef.current=false;setFormDraftDirty(false);setFormOverview(false);setFormTarget({revision:0});setActiveFormField(null);
    setFormFill({tabId:activeTab.id,page:selectedPage,document:current,adapter});
  };
  const updateFormDirty=(dirty:boolean)=>{formDirtyRef.current=dirty;setFormDraftDirty(dirty);};
  const updateFormSelection=(field:FormField|null)=>{
    setActiveFormField(field);
    if(field)setFormTarget(previous=>previous.annotationIndex===field.annotationIndex?previous:{...previous,annotationIndex:field.annotationIndex});
  };
  const closeForm=()=>{if(operation.current)return;setFormFill(null);setFormOverview(false);updateFormDirty(false);setActiveFormField(null);};
  const beginFormPosition=(field:FormField)=>{
    if(!formFill||operation.current||formDirtyRef.current||!formFill.adapter.positionFormField)return;
    try{formAuthorSelection(formFill.document,formFill.page);}catch(error){setFailure(error instanceof Error?error.message:String(error));return;}
    setFormOverview(false);setFormPosition({...formFill,field,sourceIds:workspaceCurrent.current.tabs.flatMap(tab=>tab.sourceIds)});
  };
  const beginFormTextAppearance=(field:FormField)=>{
    if(!formFill||operation.current||formDirtyRef.current||!formFill.adapter.setFormTextAppearance||!formFill.adapter.inspectFormTextAppearance)return;
    try{formAuthorSelection(formFill.document,formFill.page);}catch(error){setFailure(error instanceof Error?error.message:String(error));return;}
    setFormOverview(false);setFormTextAppearance({...formFill,field,sourceIds:workspaceCurrent.current.tabs.flatMap(tab=>tab.sourceIds)});
  };
  const beginFormFrame=(field:FormField)=>{
    if(!formFill||operation.current||formDirtyRef.current||!formFill.adapter.setFormFrame||!formFill.adapter.inspectFormFrame)return;
    try{formAuthorSelection(formFill.document,formFill.page);}catch(error){setFailure(error instanceof Error?error.message:String(error));return;}
    setFormOverview(false);setFormFrame({...formFill,field,sourceIds:workspaceCurrent.current.tabs.flatMap(tab=>tab.sourceIds)});
  };
  const beginFormProperties=(field:FormField)=>{
    if(!formFill||operation.current||formDirtyRef.current||!formFill.adapter.setFormProperties||!formFill.adapter.inspectFormProperties||!pageAllows(formFill.page,'editContent')||!pageAllows(formFill.page,'annotate'))return;
    setFormOverview(false);setFormProperties({...formFill,field,sourceIds:workspaceCurrent.current.tabs.flatMap(tab=>tab.sourceIds)});
  };
  const beginFormRemoval=(field:FormField)=>{
    if(!formFill||operation.current||formDirtyRef.current||!formFill.adapter.inspectFormRemoval||!formFill.adapter.removeFormField||!pageAllows(formFill.page,'editContent')||!pageAllows(formFill.page,'annotate'))return;
    setFormOverview(false);setFormRemoval({...formFill,field,sourceIds:workspaceCurrent.current.tabs.flatMap(tab=>tab.sourceIds)});
  };
  const removeField=async(settings:RemoveFormFieldRequest)=>{
    const request=formRemoval;if(!request||operation.current||formDirtyRef.current)throw new Error('Field removal is unavailable.');
    const currentSession=()=>{
      const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
      if(!mounted.current||!session||workspaceCurrent.current.activeId!==request.tabId||session.adapter!==request.adapter||session.history.present.pages!==request.document.pages)return null;
      return session;
    };
    if(!currentSession())throw new Error('The document changed. Reopen field removal and try again.');
    if(!pageAllows(request.page,'editContent')||!pageAllows(request.page,'annotate'))throw new Error('This document does not allow removing form fields.');
    operation.current=true;setBusy('Removing field…');
    try{await applyFormRemoval(request.adapter,request.page.sourceId,settings,id=>workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(id)),result=>{
      const session=currentSession();if(!session)return false;
      const next=commitFormReplacement(session,request.document,request.page.sourceId,result);
      setWorkspace(previous=>updateSession(previous,request.tabId,()=>next));
      setFormRemoval(null);setFormFill(null);setFormOverview(false);updateFormDirty(false);setFormTarget({revision:0});setActiveFormField(null);
      setNotice('Field removed, including its linked appearances. Undo restores the field.');return true;
    });}finally{operation.current=false;if(mounted.current)setBusy(null);}
  };
  const beginFormChoices=(field:FormField)=>{
    if(!formFill||operation.current||formDirtyRef.current||!['combo','list'].includes(field.kind)||!formFill.adapter.setFormChoices||!formFill.adapter.inspectFormChoices||!pageAllows(formFill.page,'editContent')||!pageAllows(formFill.page,'annotate'))return;
    setFormOverview(false);setFormChoices({...formFill,field,sourceIds:workspaceCurrent.current.tabs.flatMap(tab=>tab.sourceIds)});
  };
  const applyFormChoices=async(result:DocumentInfo)=>{
    const request=formChoices;
    if(!request||operation.current||formDirtyRef.current)throw new Error('Choice editing is unavailable.');
    const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
    if(!mounted.current||!session||workspaceCurrent.current.activeId!==request.tabId||session.adapter!==request.adapter||session.history.present.pages!==request.document.pages)throw new Error('The document changed. Close this dialog and try again.');
    if(!pageAllows(request.page,'editContent')||!pageAllows(request.page,'annotate'))throw new Error('This document does not allow editing field choices.');
    if(workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(result.id)))throw new Error('The choice preview must have a new source version.');
    operation.current=true;setBusy('Applying choices…');
    try{
      const next=commitFormReplacement(session,request.document,request.page.sourceId,result);
      setWorkspace(previous=>updateSession(previous,request.tabId,()=>next));
      setFormFill({...request,document:next.history.present,page:next.history.present.pages.find(page=>page.id===request.page.id)!});
      setFormChoices(null);setNotice('Choices updated. Linked appearances share these options and answers.');
    }finally{operation.current=false;if(mounted.current)setBusy(null);}
  };
  const updateFieldProperties=async(settings:SetFormPropertiesRequest)=>{
    const request=formProperties;if(!request||operation.current)throw new Error('Field properties are unavailable.');
    operation.current=true;setBusy('Updating field properties…');
    try{await applyFormProperties(request.adapter,request.page.sourceId,settings,id=>workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(id)),result=>{
      const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
      if(!mounted.current||!session||session.adapter!==request.adapter)return false;
      const next=commitFormReplacement(session,request.document,request.page.sourceId,result);
      setWorkspace(previous=>updateSession(previous,request.tabId,()=>next));
      setFormFill({...request,document:next.history.present,page:next.history.present.pages.find(page=>page.id===request.page.id)!});
      setFormProperties(null);setNotice('Field properties updated. Linked appearances share these settings.');return true;
    });}finally{operation.current=false;if(mounted.current)setBusy(null);}
  };

  const selectCanvasFormField=(page:PagePlan,field:FormField)=>{
    const request=formFill;if(!request||readingBlocked||operation.current||!pageAllows(page,"fillForms"))return;
    if(!request.document.pages.includes(page))return;
    const same=request.page.id===page.id&&activeFormField?.annotationIndex===field.annotationIndex;
    if(formDirtyRef.current&&!same)return;
    selectPage(page.id);setFormFill({...request,page});
    setFormTarget(previous=>({annotationIndex:field.annotationIndex,revision:previous.revision+1}));
  };
  const locateFormField=()=>{
    if(!formFill||readingBlocked)return;
    const bounds=activeFormField?formCanvasTargetBounds(activeFormField,formFill.page):null;
    // Center the visible widget, including upright NoRotate targets, in source
    // coordinates expected by the main viewport navigation contract.
    if(bounds){
      const point=fromDisplayPoint({x:bounds.x+bounds.width/2,y:bounds.y+bounds.height/2},formFill.page.width,formFill.page.height,formFill.page.rotation);
      navigateReading(formFill.page.id,{...point,width:0,height:0});
    }else navigateToPage(formFill.page.id);
    if(!bounds)setNotice("This field is outside the visible crop or has no visible widget. Use the full-page form overview.");
  };
  const applyFormField=async(field:FormField,value:FormFieldValue)=>{
    const request=formFill;if(!request||operation.current||!request.adapter.fillFormField)throw new Error("Form editing is unavailable.");
    operation.current=true;setBusy("Updating form…");let orphan:string|null=null;
    try {
      const result=await request.adapter.fillFormField(request.page.sourceId,{pageIndex:request.page.pageIndex,annotationIndex:field.annotationIndex,expectedValue:field.value,value});
      if(!result.id||workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(result.id)))throw new Error("The filled form must have a new source version.");
      orphan=result.id;
      const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
      if(!mounted.current||!session)return;
      const next=commitFormReplacement(session,request.document,request.page.sourceId,result);
      setWorkspace(previous=>updateSession(previous,request.tabId,()=>next));
      setFormFill({...request,document:next.history.present,page:next.history.present.pages.find(page=>page.id===request.page.id)!});
      orphan=null;setNotice("Form field updated");
    } finally {
      if(orphan)await request.adapter.closeDocument(orphan).catch(()=>{});
      operation.current=false;if(mounted.current)setBusy(null);
    }
  };
  const beginParagraphRequest = async (request:NonNullable<typeof textEdit>,fromTextDialog=false,draft?:ExistingTextDraft) => {
    if(operation.current||!pageAllows(request.page,"editContent"))return;
    const ownsRequest=()=>{
      const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
      return mounted.current&&workspaceCurrent.current.activeId===request.tabId&&session?.history.present.pages===request.document.pages&&session.adapter===request.adapter;
    };
    if(!ownsRequest())return;
    const fail=(message:string)=>{if(ownsRequest()){if(fromTextDialog)setTextEditError(message);else setFailure(message);}};
    if(!(request.run.objectPath?supportsFormParagraph(request.adapter):paragraphEditorKind(request.adapter)!==null)){fail("Paragraph editing is unavailable for this text. Use Edit text to inspect it.");return;}
    if(request.page.overlays.length&&!request.adapter.renderPagePlan){fail("Annotated paragraph previews are unavailable in this adapter.");return;}
    setFailure(null);setTextEditError(null);
    const changed=!!draft&&(draft.replacement!==draft.originalText||!!draft.font);
    if(!request.run.objectPath&&!changed){setTextParagraph(request);setTextEdit(null);return;}
    let lease=changed&&draft?.font?holdCustomFont(draft.font.id):null;
    operation.current=true;setBusy("Preparing PDF paragraph…");
    try{
      const prepared=request.run.objectPath?await prepareFormParagraph(request.adapter,request.page.sourceId,request.page.pageIndex,request.run):{adapter:request.adapter,anchor:request.run};
      if(!ownsRequest())return;
      const initialDraft=changed?await prepareParagraphDraft(prepared.adapter,request.page.sourceId,request.page.pageIndex,prepared.anchor,draft!):undefined;
      if(!ownsRequest())return;
      paragraphDraftLease.current=lease;lease=null;
      setTextParagraph({...request,run:prepared.anchor,adapter:prepared.adapter,initialDraft});setTextEdit(null);
    }catch(error){fail(errorMessage(error));}
    finally{void lease?.();operation.current=false;if(mounted.current)setBusy(null);}
  };
  const beginTextParagraph = (draft:ExistingTextDraft) => textEdit?beginParagraphRequest(textEdit,true,draft):undefined;
  const applyTextParagraph = (result: DocumentInfo,replacement?:PageContentReplacement): boolean => {
    const request=textParagraph;
    if (!request || operation.current || !mounted.current) return false;
    const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
    if (!session) return false;
    if(workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(result.id)))throw new Error("Paragraph preview must return a new source version.");
    const kind=paragraphEditorKind(request.adapter);
    if(kind&&supportsOccurrenceParagraph(request.adapter,kind)&&!replacement)throw new Error("Paragraph preview omitted its page mapping.");
    if(replacement&&replacement.document!==result)throw new Error("Paragraph preview changed before Apply.");
    const next=replacement?commitPageReplacement(session,request.document,request.page,replacement):commitTextReplacement(session,request.page,result);
    setWorkspace(value=>updateSession(value,request.tabId,()=>next));
    setTextParagraph(null); setTool("edit"); setNotice("PDF paragraph updated");
    return true;
  };
  const beginTextGroup = async () => {
    if(!textEdit || textEdit.run.objectPath || operation.current || !supportsTextGroup(textEdit.adapter))return;
    const request={...textEdit,runs:[] as EditableTextRun[],loading:true};
    const generation=++groupGeneration.current;
    setTextGroup(request);setTextEdit(null);setTextEditError(null);setTextGroupError(null);
    try {
      const list=supportsOccurrenceGroup(request.adapter)?request.adapter.listOccurrenceTextRuns!:request.adapter.listTextRuns!;
      const result=await list(request.page.sourceId,request.page.pageIndex);
      if(mounted.current && generation===groupGeneration.current){
        setTextGroup({...request,runs:result.runs,loading:false});
        if(result.reason)setTextGroupError(result.reason);
      }
    } catch(error) {
      if(mounted.current && generation===groupGeneration.current){setTextGroup({...request,loading:false});setTextGroupError(errorMessage(error));}
    }
  };
  const applyTextGroup = async (preview:TextGroupPreview,replacement:string) => {
    const request=textGroup;
    if(!request || operation.current || !supportsTextGroup(request.adapter))return;
    const editingAdapter=request.adapter;
    operation.current=true;setBusy("Updating PDF text…");setTextGroupError(null);
    let orphanSource:string|null=null;
    try {
      let occurrenceResult:PageContentReplacement|undefined;
      let result:DocumentInfo;
      if(supportsOccurrenceGroup(editingAdapter)){
        const target=pageReplacementRequest(request.document,request.page);
        occurrenceResult=await editingAdapter.replaceTextGroupOccurrence!(target.sourceId,target.occurrences,target.selectedOccurrence,preview.objectIndices,preview.text,replacement);
        result=occurrenceResult.document;
      }else result=await editingAdapter.replaceTextGroup!(request.page.sourceId,request.page.pageIndex,preview.objectIndices,preview.text,replacement);
      const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
      const borrowed=workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(result.id));
      if(result.id&&!borrowed)orphanSource=result.id;
      if(!result.id||borrowed)throw new Error("The text edit must return a new source version.");
      if(!mounted.current || !session)return;
      const apply=(tab:DocumentSession)=>occurrenceResult?commitPageReplacement(tab,request.document,request.page,occurrenceResult):commitTextReplacement(tab,request.page,result);
      apply(session);
      setWorkspace(value=>updateSession(value,request.tabId,apply));
      orphanSource=null;groupGeneration.current++;setTextGroup(null);setTool("edit");setNotice("PDF text updated");
    } catch(error) {if(mounted.current)setTextGroupError(errorMessage(error));}
    finally {
      if(orphanSource)await editingAdapter.closeDocument(orphanSource).catch(()=>{});
      operation.current=false;if(mounted.current)setBusy(null);
    }
  };
  const addText = (pageId: string, point: InkPoint) => {
    if(!pageAllows(current?.pages.find(page=>page.id===pageId),"annotate"))return;
    const id = uniqueId("text");
    edit((document) => ({
      ...document, selectedPageId: pageId, selectedOverlayId: id,
      pages: document.pages.map((page) => page.id === pageId
        ? { ...page, overlays: [...page.overlays, { type: "text", id, x: point.x, y: point.y, text: "Type here", fontSize: 18, color: "#2D2A26" }] } : page),
    }));
    setNewTextId(id);
    setTool("select");
  };
  const addTextArea = (pageId:string, rect:AnnotationRect) => {
    if(!pageAllows(current?.pages.find(page=>page.id===pageId),"annotate"))return;
    if(blocked || adapter.kind!=="native" || !workspace.activeId || !current?.pages.some(page=>page.id===pageId)) return;
    setFontPicker({tabId:workspace.activeId,pageId,overlayId:uniqueId("area"),text:"Aa",fontSize:18,
      shaping:{version:1,direction:"ltr",ligatures:true},area:{version:1,width:rect.width,height:rect.height,inset:4,lineSpacing:1.2,alignment:"left"},createAt:{x:rect.x,y:rect.y}});
  };
  const resizeTextArea = (pageId:string,id:string,rect:AnnotationRect) => {
    if(!pageAllows(current?.pages.find(page=>page.id===pageId),"annotate"))return;
    const overlay=current?.pages.find(page=>page.id===pageId)?.overlays.find(value=>value.id===id);
    if(blocked || overlay?.type!=="text" || !overlay.area || !Object.values(rect).every(Number.isFinite)) return;
    if(rect.x===overlay.x && rect.y===overlay.y && rect.width===overlay.area.width && rect.height===overlay.area.height) return;
    edit(document=>updateOverlay(document,pageId,id,value=>value.type==="text"&&value.area?{...value,x:rect.x,y:rect.y,area:{...value.area,width:rect.width,height:rect.height}}:value));
  };
  const transformImage=(pageId:string,before:import("./editor/types").ImageOverlay,after:import("./editor/types").ImageOverlay)=>{
    if(before===after||after.id!==before.id||after.assetId!==before.assetId)return;
    const box=imageBounds(after);
    if(!Object.values(box).every(Number.isFinite)||box.width<=0||box.height<=0)return;
    edit(document=>{
      const page=document.pages.find(page=>page.id===pageId);
      if(!page||!pageAllows(page,"annotate")||page.overlays.find(overlay=>overlay.id===before.id)!==before||box.x<-.001||box.y<-.001||box.x+box.width>page.width+.001||box.y+box.height>page.height+.001)return document;
      return updateOverlay(document,pageId,before.id,()=>after);
    });
  };
  const addHighlight = (pageId: string, rects: AnnotationRect[]) => {
    if(!pageAllows(current?.pages.find(page=>page.id===pageId),"annotate"))return;
    if (!rects.length) return;
    const id = uniqueId("highlight");
    edit(document => ({...document, selectedPageId:pageId, selectedOverlayId:id,
      pages:document.pages.map(page=>page.id===pageId ? {...page,overlays:[...page.overlays,{type:"highlight",id,rects,color:"#FFE066"}]} : page)}));
    setNotice("Highlight added");
  };
  const addComment = (pageId: string, point: InkPoint) => {
    if(!pageAllows(current?.pages.find(page=>page.id===pageId),"annotate"))return;
    const id = uniqueId("comment");
    edit(document => ({...document, selectedPageId:pageId, selectedOverlayId:id,
      pages:document.pages.map(page=>page.id===pageId ? {...page,overlays:[...page.overlays,{type:"comment",id,x:Math.min(point.x,Math.max(0,page.width-24)),y:Math.min(point.y,Math.max(0,page.height-24)),text:"",color:"#FFE066"}]} : page)}));
    setNewTextId(id); setTool("select");
  };
  const navigateToAnnotation = (pageId: string, overlay: Overlay) => {
    if (blocked) return;
    setTool("select"); setPendingSignature(null);updateSearch({open:false});
    const rect = overlay.type === "highlight" ? overlay.rects[0] : overlay.type === "comment" ? {x:overlay.x,y:overlay.y,width:24,height:24} : undefined;
    navigateReading(pageId,rect,overlay.id);
  };
  const addDrawing = (pageId: string, path: InkPoint[]) => {
    if(!pageAllows(current?.pages.find(page=>page.id===pageId),"annotate"))return;
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
    if(!pageAllows(current?.pages.find(page=>page.id===pageId),"annotate"))return;
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
    if(!pageAllows(current?.pages.find(page=>page.id===pageId),"annotate"))return;
    if (!history || (blocked && phase !== "end")) return;
    if (phase === "start") { dragOverlay.current = { document: history.present, pageId, x, y, id }; return; }
    const start = dragOverlay.current;
    if (!start || start.id !== id || start.pageId !== pageId) return;
    if (phase === "end") {
      setHistory((value) => value && planDigest(value.present) !== planDigest(start.document)
        ? commit({...value,present:start.document},()=>value.present) : value);
      dragOverlay.current = null;
      return;
    }
    const dx = x - start.x, dy = y - start.y;
    const sourcePage = start.document.pages.find((page) => page.id === pageId);
    const sourceOverlay = sourcePage?.overlays.find((overlay) => overlay.id === id);
    if (!sourceOverlay) return;
    setHistory((value) => value ? { ...value, present: updateOverlay(value.present, pageId, id, () => sourceOverlay.type === "image" ? moveImage(sourceOverlay,sourceOverlay.x+dx,sourceOverlay.y+dy,sourcePage!.width,sourcePage!.height) : (sourceOverlay.type === "text" || sourceOverlay.type === "comment")
      ? sourceOverlay.type === "text" && sourceOverlay.area ? moveArea(sourceOverlay,sourceOverlay.x+dx,sourceOverlay.y+dy,sourcePage!.width,sourcePage!.height) : { ...sourceOverlay, x: sourceOverlay.x + dx, y: sourceOverlay.y + dy }
      : sourceOverlay.type === "highlight" ? sourceOverlay : { ...sourceOverlay, paths: sourceOverlay.paths.map((path) => path.map((point) => ({ x: point.x + dx, y: point.y + dy }))) }) } : value);
  };

  const chooseTool = (next: Tool) => {
    if(!toolAllowed(selectedPage,next))return;
    imagePlacement.cancel();
    setTool(next); setPendingSignature(null);
    if (selectedPage) selectOverlay(selectedPage.id, null);
    if (next === "signature") setSignatureOpen(true);
    if (next === "image") void imagePlacement.begin();
  };
  const changeHistory = useCallback((direction: "undo" | "redo") => {
    if (blocked) return;
    setHistory((value) => value ? (direction === "undo" ? undo(value) : redo(value)) : value);
  }, [blocked, setHistory]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (readingBlocked || decision.current || event.defaultPrevented || event.isComposing || event.keyCode === 229) return;
      if(textParagraph&&event.key==="Escape"){event.preventDefault();setTextParagraph(null);return;}
      if(imageReplacement&&event.key==="Escape"){event.preventDefault();setImageReplacement(null);return;}
      const command = event.ctrlKey || event.metaKey;
      const key = event.key.toLowerCase();
      const typing = (event.target as HTMLElement)?.closest?.("input, textarea, select, [contenteditable='true']");
      const interactive=(event.target as HTMLElement)?.closest?.("button, a, [role='button'], [role='menuitem']");
      if(event.altKey&&!event.ctrlKey&&!event.metaKey&&!event.shiftKey&&!typing&&(event.key==='ArrowLeft'||event.key==='ArrowRight')){
        event.preventDefault();travelReading(event.key==='ArrowLeft'?'back':'forward');return;
      }
      if(blocked)return;
      if(!typing&&imagePlacement.pending&&(event.key==="Escape"||(event.key==="Enter"&&!interactive))){
        event.preventDefault();if(event.key==="Enter")imagePlacement.place();else {imagePlacement.cancel();setTool("select");}return;
      }
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
  }, [textParagraph,imageReplacement, blocked, readingBlocked, changeHistory, closeTab, edit, exportPdf, openPdf, openSearch, openPrint, selectOverlay, selectedOverlay, selectedPage, switchTab, workspace,imagePlacement,travelReading]);

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
    if(!canArrange)return;
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
    if(!canArrange)return;
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
  const displaySize = selectedPage ? pageVisibleRect(selectedPage) : null;
  const chooseFont = (info: FontInfo) => {
    const request = fontPicker;
    const tab = workspaceCurrent.current.tabs.find(tab => tab.id === request?.tabId);
    if(mounted.current && request?.createAt && request.area && tab?.history.present.pages.some(page=>page.id===request.pageId)) {
      const overlay:TextOverlay={type:"text",id:request.overlayId,...request.createAt,text:"",fontSize:request.fontSize,color:"#2D2A26",fontId:info.id,shaping:request.shaping,area:{...request.area}};
      retainCustomFonts(new Set([...workspaceFontIds(workspaceCurrent.current),info.id]));
      setWorkspace(value=>updateSession(value,request.tabId,session=>({...session,history:commit(session.history,document=>({...document,selectedPageId:request.pageId,selectedOverlayId:overlay.id,pages:document.pages.map(page=>page.id===request.pageId?{...page,overlays:[...page.overlays,overlay]}:page)}))})));
      setFontPicker(null);setTool("select");setNewTextId(overlay.id);return;
    }
    const overlay = tab?.history.present.pages.find(page => page.id === request?.pageId)?.overlays.find(overlay => overlay.id === request?.overlayId);
    if (!mounted.current || !request || !tab || overlay?.type !== "text") throw new Error("This text box is no longer available.");
    if (overlay.fontId === info.id) { setFontPicker(null); return; }
    // Own the chosen resource before the picker drops its temporary preview lease.
    retainCustomFonts(new Set([...workspaceFontIds(workspaceCurrent.current), info.id]));
    setWorkspace(value => updateSession(value, request.tabId, session => ({ ...session,
      history: commit(session.history, document => updateOverlay(document, request.pageId, request.overlayId, value => value.type === "text" ? { ...value, fontId: info.id } : value)),
    })));
    setFontPicker(null);
  };
  const renderImageViewport=(canvas?:CanvasImageView,paragraph?:CanvasParagraphView)=>current&&activeTab?<DocumentViewport canvasParagraph={paragraph?{...paragraph,enabled:paragraph.enabled&&!readingBlocked}:undefined} canvasImage={canvas?{...canvas,enabled:canvas.enabled&&!readingBlocked}:undefined} nativeNotes={nativeReview.notes} onOpenNativeComment={openNativeComment} onMoveNativeComment={moveNativeComment} key={activeTab!.id} captureViewRef={captureViewRef} onNavigationComplete={completeNavigation} initialScrollPosition={scrollPositions.current.get(activeTab!.id)} onScrollPositionChange={position => { scrollPositions.current.set(activeTab!.id, position); recovery.schedule(); }} adapter={adapter} pages={current.pages} selectedPageId={selectedPage?.id ?? null} selectedOverlayId={current.selectedOverlayId}
          zoom={zoom} onZoomChange={setZoom} viewMode={preferences.viewMode} tool={formFill?"form":tool} penColor={preferences.penColor} penWidth={preferences.penWidth}
          imageAspectLocked={imageAspectLocked} onTransformPageImage={transformImage}
          pendingImagePlacement={imagePlacement.pending} onMoveImage={imagePlacement.move} onPlaceImage={imagePlacement.place}
          formSelection={formFill?{pageId:formFill.page.id,annotationIndex:activeFormField?.annotationIndex}:undefined} formDirty={formDraftDirty} onFormAnchor={receiveFormAnchor} onSelectFormField={selectCanvasFormField}
          pendingSignature={pendingSignature} interactionDisabled={readingBlocked} navigationRequest={navigationRequest} searchMatches={searchState.open?search.matches:[]} activeSearchMatchId={searchMatch?.id}
          onSelectPage={selectPage} onEditImage={(pageId,image)=>{
            if(blocked||!activeTab||!current||!adapter.listExistingImages||!adapter.previewImageReplacementOccurrence||!adapter.previewImageEditOccurrence||!adapter.renderPagePlan)return;
            const page=current.pages.find(value=>value.id===pageId);if(!page)return;
            try{pageReplacementRequest(current,page);}catch(cause){setFailure(cause instanceof Error?cause.message:String(cause));return;}
            navigateReading(pageId,image.bounds);updateSearch({open:false});setImageReplacement({tabId:activeTab.id,document:current,page,adapter,image,sourceIds:workspace.tabs.flatMap(tab=>tab.sourceIds)});
          }} onEditText={beginTextEdit} onSelectOverlay={selectOverlay} onAddText={addText} onAddTextArea={addTextArea} onResizeTextArea={resizeTextArea} onPlaceSignature={placeSignature} onMoveOverlay={moveOverlay} onDraw={addDrawing} onHighlight={addHighlight} onAddComment={addComment}/>:null;
  const imageWorkspace=imageReplacement?<ImageReplacementDialog key={`${imageReplacement.tabId}:${imageReplacement.page.sourceId}:${imageReplacement.image.selectionId}`} adapter={imageReplacement.adapter} document={current!} canvasZoom={zoom} renderCanvas={renderImageViewport} onLocate={()=>navigateReading(imageReplacement.page.id,imageReplacement.image.bounds)} page={imageReplacement.page} image={imageReplacement.image} sourceIds={imageReplacement.sourceIds} isSourceRetained={id=>workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(id))} onCancel={()=>setImageReplacement(null)} onApply={result=>{
      const request=imageReplacement;if(!mounted.current||operation.current)return false;
      const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);if(!session||session.adapter!==request.adapter)return false;
      if(workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(result.document.id)))throw new Error("Replacement must return a new source version.");
      const next=commitPageReplacement(session,request.document,request.page,result);
      setWorkspace(value=>updateSession(value,request.tabId,()=>next));setImageReplacement(null);setNotice("Image edited. Use Undo to restore it, or save a copy when finished.");return true;
    }}/>:renderImageViewport();
  const paragraphWorkspace=imageReplacement?imageWorkspace:<ParagraphViewportHost key={activeTab?.id} session={textParagraph?{pageId:textParagraph.page.id,sessionKey:`${textParagraph.tabId}:${textParagraph.page.id}:${textParagraph.page.sourceId}`}:undefined} renderViewport={view=>renderImageViewport(undefined,view)}>{textParagraph ? (()=>{const ParagraphDialog=paragraphEditorKind(textParagraph.adapter)==="styled"?StyledParagraphDialog:TextParagraphDialog;return <ParagraphDialog key={`${textParagraph.tabId}:${textParagraph.page.id}:${textParagraph.page.sourceId}`} canvas={{pageId:textParagraph.page.id,sessionKey:`${textParagraph.tabId}:${textParagraph.page.id}:${textParagraph.page.sourceId}`,zoom,renderCanvas:view=>renderImageViewport(undefined,view),onLocate:()=>navigateReading(textParagraph.page.id,textParagraph.run.bounds),onModalChange:setParagraphModal}} occurrence={paragraphOccurrence} crop={textParagraph.page.crop} adapter={textParagraph.adapter} sourceId={textParagraph.page.sourceId} pageIndex={textParagraph.page.pageIndex} pageWidth={textParagraph.page.width} pageHeight={textParagraph.page.height} overlays={textParagraph.page.overlays} anchor={textParagraph.run} initialDraft={textParagraph.initialDraft} onApply={applyTextParagraph} onCancel={()=>setTextParagraph(null)}/>;})():null}</ParagraphViewportHost>;
  const dialogs = <>
    {ocr&&<OcrDialog document={ocr.document} adapter={ocr.adapter} knownSourceIds={()=>new Set(workspaceCurrent.current.tabs.flatMap(tab=>tab.sourceIds))} onAdopt={adoptOcr} onClose={()=>setOcr(null)}/>}
    {passwords.prompt&&<PasswordDialog {...passwords.prompt} onSubmit={passwords.submit} onCancel={passwords.cancel}/>}
    {blankRequest&&<BlankPageDialog inserting={!!blankRequest.tabId} onCreate={createBlank} onClose={()=>setBlankRequest(null)}/>}
    {imageRequest&&<ImagePagesDialog inserting={!!imageRequest.tabId} onPick={()=>nativeAdapter.pickImages!()} onCreate={createImages} onClose={()=>setImageRequest(null)}/>}
    {bookmarkDialog&&current&&<BookmarkDialog bookmark={current.bookmarks?.find(b=>b.id===bookmarkDialog.id)} pages={current.pages} currentPageId={selectedPage?.id??null} onClose={()=>setBookmarkDialog(null)} onSave={(title,pageId)=>{
      const next=bookmarkDialog.id?editBookmark(current,bookmarkDialog.id,{title,...(pageId?{pageId}:{})}):addBookmark(current,title,pageId??selectedPage!.id,bookmarkDialog.parentId);
      setHistory(value=>value?commit(value,()=>next):value);setBookmarkDialog(null);
    }}/>}
    {pageJump&&current&&<PageJumpDialog current={selectedIndex} total={current.pages.length} onClose={()=>setPageJump(false)} onGo={index=>{updateSearch({open:false});navigateToPage(current.pages[index].id);setPageJump(false);}}/>}
    {fontPicker && <FontPicker title={fontPicker.createAt ? "Choose a font for your text area" : undefined} fontSize={fontPicker.fontSize} shaping={fontPicker.shaping} area={fontPicker.area} currentFontId={fontPicker.fontId} text={fontPicker.text} onChoose={chooseFont} onClose={() => setFontPicker(null)}/>}
    {textEdit && <ExistingTextDialog run={textEdit.run} busy={!!busy} error={textEditError} onApply={applyTextEdit} onDraftChange={()=>setTextEditError(null)} onEditParagraph={(textEdit.run.objectPath?supportsFormParagraph(textEdit.adapter):paragraphEditorKind(textEdit.adapter)!==null) ? draft=>void beginTextParagraph(draft) : undefined} onEditTogether={!textEdit.run.objectPath && supportsTextGroup(textEdit.adapter) ? ()=>void beginTextGroup() : undefined} onCancel={()=>{if(!operation.current){setTextEdit(null);setTextEditError(null);}}}/> }
    {textGroup && <TextGroupDialog anchor={textGroup.run} runs={textGroup.runs} loading={textGroup.loading} busy={!!busy} error={textGroupError}
      onInspect={indices=>(supportsOccurrenceGroup(textGroup.adapter)?textGroup.adapter.inspectOccurrenceTextGroup!:textGroup.adapter.inspectTextGroup!)(textGroup.page.sourceId,textGroup.page.pageIndex,indices)} onApply={applyTextGroup} onDraftChange={()=>setTextGroupError(null)}
      onCancel={()=>{if(!operation.current){groupGeneration.current++;setTextGroup(null);setTextGroupError(null);}}}/>}
    {(recovery.pending||recovery.error)&&<RecoveryDialog count={recovery.pending?.tabs.length??0} error={recovery.error} busy={recovery.working} onRestore={()=>void recovery.restore()} onDiscard={()=>void recovery.discard()} onSkip={recovery.skip}/>}
    {organizer&&<PageOrganizerDialog document={organizer.document} adapter={organizer.adapter} onApply={applyOrganization} onExtract={extractOrganizedPages} onClose={()=>{if(!operation.current)setOrganizer(null);}}/>}
    {extraction&&<ExtractPagesDialog pages={extraction.pages} currentPageId={extraction.currentPageId} onClose={()=>setExtraction(null)} onExport={extractPages}/>}
    {securityCopy&&<ProtectedCopyDialog {...securityCopy} adapter={adapter} onBusyChange={value=>{operation.current=value;}} onClose={()=>{if(!operation.current)setSecurityCopy(null);}} onComplete={path=>{setNotice(`Saved a copy with new security settings to ${path}`);setSecurityCopy(null);}}/>}
    {textRemoval&&<TextRemovalDialog request={textRemoval} isCurrent={()=>{
      const value=workspaceCurrent.current,tab=value.tabs.find(tab=>tab.id===textRemoval.tabId);
      return mounted.current&&value.activeId===textRemoval.tabId&&tab?.adapter===textRemoval.adapter&&tab.history===textRemoval.history;
    }} isBorrowed={id=>workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(id))} onCancel={()=>setTextRemoval(null)} onApply={info=>{
      const value=workspaceCurrent.current,session=value.tabs.find(tab=>tab.id===textRemoval.tabId);
      if(!mounted.current||operation.current||value.activeId!==textRemoval.tabId||!session||value.tabs.some(tab=>tab.sourceIds.includes(info.id)))return false;
      const next=commitTextRemoval(session,textRemoval,info),updated=updateSession(value,textRemoval.tabId,()=>next);
      workspaceCurrent.current=updated;setWorkspace(updated);setTextRemoval(null);setTool('select');setNotice(`${textRemoval.mode==='content'?'Content':'Text'} removed. Use Undo to restore it, or save a copy.`);return true;
    }}/>} 
    {pdfSave&&<PdfSaveProgressDialog view={pdfSave} onCancel={()=>void cancelPdfSave()}/>}
    {textExport&&<ExportTextDialog pages={textExport.pages} currentPageId={textExport.currentPageId} onBusyChange={value=>{operation.current=value;}} onClose={()=>{if(!operation.current)setTextExport(null);}} onComplete={result=>setNotice(`Exported text to ${result.path}`)}/>}
    {pdfSplit&&<SplitPdfDialog {...pdfSplit} onBusyChange={value=>{operation.current=value;}} onClose={()=>{if(!operation.current)setPdfSplit(null);}} onComplete={result=>{setNotice(`Split into ${result.count} PDF${result.count===1?"":"s"} in ${result.folder}`);setPdfSplit(null);}}/>}
    {imageExport&&<ExportImagesDialog pages={imageExport.pages} currentPageId={imageExport.currentPageId} onBusyChange={value=>{operation.current=value;}} onClose={()=>{if(!operation.current)setImageExport(null);}} onComplete={result=>{setNotice(`Exported ${result.count} page image${result.count===1?"":"s"} to ${result.folder}`);setImageExport(null);}}/>}
    {cropRequest&&<CropPageDialog adapter={cropRequest.adapter} page={cropRequest.page} onCancel={()=>setCropRequest(null)} onApply={crop=>{
      const request=cropRequest;
      setWorkspace(workspace=>updateSession(workspace,request.tabId,tab=>tab.history.present!==request.document?tab:{...tab,history:commit(tab.history,document=>cropPage(document,request.page.id,crop))}));
      setCropRequest(null);setNotice("Page crop applied. Save a copy when finished.");
    }}/>} 
    {documentProperties&&<DocumentPropertiesDialog adapter={documentProperties.adapter} sourceId={documentProperties.document.pages[0].sourceId} metadata={documentProperties.document.metadata} canEdit={documentProperties.document.pages.every(page=>pageAllows(page,'editContent'))} onClose={()=>setDocumentProperties(null)} onApply={change=>{
      const request=documentProperties,session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
      if(operation.current||workspaceCurrent.current.activeId!==request.tabId||!session||session.adapter!==request.adapter||session.history.present!==request.document)throw Error('The document changed. Reopen document properties and try again.');
      if(change!==undefined)validateMetadata(change);
      const next={...request.document,metadata:change?{...change}:undefined};
      if(!allowsWorkspaceEdit(request.document,next))throw Error('This PDF does not permit changing document properties.');
      setWorkspace(previous=>updateSession(previous,request.tabId,tab=>tab.adapter===request.adapter&&tab.history.present===request.document?{...tab,history:commit(tab.history,()=>next)}:tab));
      setDocumentProperties(null);setNotice('Document properties updated. Save a PDF copy to write the changes.');
    }}/>}
    {formProperties&&<FormPropertiesDialog {...formProperties} onClose={()=>{if(!operation.current)setFormProperties(null);}} onApply={updateFieldProperties}/>}
    {formRemoval&&<FormRemovalDialog {...formRemoval} onClose={()=>{if(!operation.current)setFormRemoval(null);}} onApply={removeField}/>}
    {formChoices&&<FormChoicesDialog {...formChoices} isBorrowed={id=>workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(id))} onApply={applyFormChoices} onClose={()=>{if(!operation.current)setFormChoices(null);}}/>}
    {formTextAppearance&&<FormTextAppearanceDialog {...formTextAppearance} onCancel={()=>setFormTextAppearance(null)} onApply={result=>{
      const request=formTextAppearance;
      if(!mounted.current||operation.current)return false;
      const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
      if(!session||session.adapter!==request.adapter)return false;
      if(workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(result.document.id)))throw new Error("Text appearance must return a new source version.");
      const next=commitPageReplacement(session,request.document,request.page,result);
      setWorkspace(value=>updateSession(value,request.tabId,()=>next));
      setFormTextAppearance(null);closeForm();setTool("select");setNotice("Field appearance applied. Save a copy when finished.");return true;
    }}/>} 
    {formFrame&&<FormFrameDialog {...formFrame} isBorrowed={id=>workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(id))} onCancel={()=>setFormFrame(null)} onApply={result=>{
      const request=formFrame;
      if(!mounted.current||operation.current)return false;
      const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
      if(!session||session.adapter!==request.adapter)return false;
      if(workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(result.document.id)))throw new Error("Border editing must return a new source version.");
      const next=commitPageReplacement(session,request.document,request.page,result);
      setWorkspace(value=>updateSession(value,request.tabId,()=>next));
      setFormFrame(null);closeForm();setTool("select");setNotice("Field border and background applied. Save a copy when finished.");return true;
    }}/>} 
    {formPosition&&<FormPositionDialog {...formPosition} onCancel={()=>setFormPosition(null)} onApply={result=>{
      const request=formPosition;
      if(!mounted.current||operation.current)return false;
      const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
      if(!session||session.adapter!==request.adapter)return false;
      if(workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(result.document.id)))throw new Error("Field positioning must return a new source version.");
      const next=commitPageReplacement(session,request.document,request.page,result);
      setWorkspace(value=>updateSession(value,request.tabId,()=>next));
      setFormPosition(null);closeForm();setTool("select");setNotice("Field position applied. Save a copy when finished.");return true;
    }}/>} 
    {formAuthor&&<FormAuthorDialog {...formAuthor} onCancel={()=>setFormAuthor(null)} onApply={result=>{
      const request=formAuthor;
      if(!mounted.current||operation.current)return false;
      const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
      if(!session||session.adapter!==request.adapter)return false;
      if(workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(result.document.id)))throw new Error("Field creation must return a new source version.");
      const next=commitPageReplacement(session,request.document,request.page,result);
      setWorkspace(value=>updateSession(value,request.tabId,()=>next));
      setFormAuthor(null);setTool("select");setNotice("Field created. Use Fill forms to enter a value, or Undo to remove it.");
      return true;
    }}/>} 
    {pageReplacement&&<PageReplacementDialog adapter={pageReplacement.adapter} document={pageReplacement.document} page={pageReplacement.page} sourceIds={pageReplacement.sourceIds} onCancel={()=>setPageReplacement(null)} onApply={result=>{
      const request=pageReplacement;
      if(!mounted.current||operation.current)return false;
      const session=workspaceCurrent.current.tabs.find(tab=>tab.id===request.tabId);
      if(!session||session.adapter!==request.adapter)return false;
      if(workspaceCurrent.current.tabs.some(tab=>tab.sourceIds.includes(result.document.id)))throw new Error("Replacement must return a new source version.");
      const next=commitPageReplacement(session,request.document,request.page,result);
      setWorkspace(value=>updateSession(value,request.tabId,()=>next));
      setPageReplacement(null);setTool("select");setNotice("Page content replaced. Use Undo to restore it, or save a copy when finished.");
      return true;
    }}/>} 
    {nativeComment&&<NativeCommentDialog comment={nativeComment.comment} page={nativeComment.page} pageNumber={nativeComment.document.pages.findIndex(page=>page.id===nativeComment.page.id)+1} reason={!pageAllows(nativeComment.page,"annotate")?"This PDF does not permit changing annotations.":!nativeComment.adapter.editNativeComment?"Comment editing is unavailable in this build.":nativeComment.comment.reason} onApply={async changes=>{await mutateNativeComment(changes);}} onDelete={nativeComment.adapter.deleteNativeComment?async()=>{await mutateNativeComment(null);}:undefined} onClose={()=>{if(!operation.current)setNativeComment(null);}}/>}
    {formFill&&formOverview&&<FormFillDialog page={formFill.page} pageNumber={formFill.document.pages.findIndex(page=>page.id===formFill.page.id)+1} adapter={formFill.adapter} selectedAnnotationIndex={activeFormField?.annotationIndex??formTarget.annotationIndex} onDraftChange={updateFormDirty} onSelectionChange={updateFormSelection} onPosition={formFill.adapter.positionFormField&&pageAllows(formFill.page,"editContent")&&pageAllows(formFill.page,"annotate")?beginFormPosition:undefined} onChoices={formFill.adapter.setFormChoices&&formFill.adapter.inspectFormChoices&&pageAllows(formFill.page,'editContent')&&pageAllows(formFill.page,'annotate')?beginFormChoices:undefined} onRemove={formFill.adapter.removeFormField&&formFill.adapter.inspectFormRemoval&&pageAllows(formFill.page,'editContent')&&pageAllows(formFill.page,'annotate')?beginFormRemoval:undefined} onProperties={formFill.adapter.setFormProperties&&formFill.adapter.inspectFormProperties&&pageAllows(formFill.page,'editContent')&&pageAllows(formFill.page,'annotate')?beginFormProperties:undefined} onTextAppearance={formFill.adapter.setFormTextAppearance&&formFill.adapter.inspectFormTextAppearance&&pageAllows(formFill.page,"editContent")&&pageAllows(formFill.page,"annotate")?beginFormTextAppearance:undefined} onFrameAppearance={formFill.adapter.setFormFrame&&formFill.adapter.inspectFormFrame&&pageAllows(formFill.page,"editContent")&&pageAllows(formFill.page,"annotate")?beginFormFrame:undefined} onApply={applyFormField} onClose={()=>{if(!operation.current){setFormOverview(false);updateFormDirty(false);}}}/>}
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
      <div className="empty-actions"><button className="button primary large" disabled={blocked} onClick={openPdf}><FolderOpen size={18}/> Open a PDF</button><button className="button large" disabled={blocked} onClick={()=>openBlank(false)}>New PDF</button><button className="button large" disabled={blocked} onClick={()=>openImages(false)}>Images to PDF</button><button className="button large" disabled={blocked} onClick={openDemo}>Explore demo</button></div>
      {recovery.warning&&<p className="error-note" role="status">{recovery.warning}</p>}{!recovery.ready&&!recovery.pending&&!recovery.error&&<p className="busy-note">Checking for recovered work…</p>}{busy && <p className="busy-note">{busy}</p>}{failure && <p className="error-note" role="alert">{failure}</p>}
      <p className="privacy-note"><span>●</span> Local editing · Source files remain untouched</p>
    </section>{dialogs}
  </main>;

  return <div className="app-shell">
    <header className="titlebar">
      <div className="brand"><span className="folio-mark small">F</span><span>Folio</span></div>
      <div ref={tabStripRef} className="document-tabs" role="tablist" aria-label="Open documents">
        {workspace.tabs.map(tab => <div key={tab.id} className={"document-tab " + (tab.id === workspace.activeId ? "active" : "")}>
          <button id={"tab-" + tab.id} role="tab" aria-label={tab.history.present.name} aria-selected={tab.id === workspace.activeId} tabIndex={tab.id === workspace.activeId ? 0 : -1} disabled={blocked}
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
          <button className="tab-close" aria-label={"Close " + tab.history.present.name} title="Close tab (Ctrl+W)" disabled={blocked} onClick={() => void closeTab(tab.id)}><X size={13}/></button>
        </div>)}
        <button className="icon-button new-document" aria-label="Open PDF in new tab" title="Open PDF in new tab (Ctrl+O)" disabled={blocked} onClick={openPdf}><Plus size={16}/></button>
      </div>
      <div className="titlebar-actions"><button className="icon-button" aria-label="Settings" title="Settings" disabled={blocked} onClick={() => setSettingsOpen(true)}><Settings size={18}/></button></div>
    </header>
    <div className="toolbar" role="toolbar" aria-label="Document tools">
      <div className="toolbar-actions" role="group" aria-label="Document actions">
      <div className="tool-group"><button className="tool-button" disabled={blocked} onClick={()=>openImages(false)}>Images to PDF</button><button className="tool-button" disabled={blocked} onClick={()=>openBlank(false)}><Plus size={17}/><span>New PDF</span></button><button className="tool-button" onClick={openPdf} disabled={blocked} title="Open PDF (Ctrl+O)"><FolderOpen size={17}/><span>Open</span></button><button className="tool-button" onClick={addPdf} disabled={blocked || !canArrange || adapter.kind === "demo"} title={canArrange?"Add another PDF":"This PDF does not permit combining pages."}><FilePlus2 size={17}/><span>Add PDF</span></button></div>
      <div className="separator"/>
      <div className="tool-group"><button className="icon-button" aria-label="Undo" title={`Undo (Ctrl+Z). Keeps up to ${MAX_DOCUMENT_HISTORY_ACTIONS} document actions.`} disabled={blocked || !history?.past.length} onClick={() => changeHistory("undo")}><Undo2 size={18}/></button><button className="icon-button" aria-label="Redo" disabled={blocked || !history?.future.length} onClick={() => changeHistory("redo")}><Redo2 size={18}/></button></div>
      <div className="toolbar-spacer"/>
      <div className="zoom-control"><button aria-label="Zoom out" disabled={readingBlocked} onClick={() => setReadingZoom((value) => Math.max(50, value - 10))}><Minus size={15}/></button><button className="zoom-value" title="Reset zoom to 100% (Ctrl+scroll to zoom)" disabled={readingBlocked} onClick={() => setReadingZoom(100)}>{zoom}%</button><button aria-label="Zoom in" disabled={readingBlocked} onClick={() => setReadingZoom((value) => Math.min(200, value + 10))}><Plus size={15}/></button></div>
      <button className="icon-button" aria-label="Find" title="Find (Ctrl+F)" disabled={blocked} onClick={openSearch}><Search size={18}/></button>
      <button className="icon-button" aria-label="Print" title="Print (Ctrl+P)" disabled={blocked||!canPrint||adapter.kind==="demo"} onClick={openPrint}><Printer size={18}/></button>
      <SaveCopyButton key={activeTab!.id} onProperties={adapter.inspectDocumentProperties?()=>{if(!blocked&&current&&activeTab)setDocumentProperties({tabId:activeTab.id,document:current,adapter});}:undefined} onSecurity={adapter.kind==="native"?()=>{if(blocked||!current)return;if(current.bookmarkError){setFailure("Unreadable bookmarks must be discarded explicitly in the Bookmarks panel before saving.");return;}setSecurityCopy({pages:current.pages,...(current.metadata!==undefined?{metadata:current.metadata}:{}),...(current.bookmarks!==undefined?{bookmarks:filterBookmarks(current.bookmarks,current.pages)}:{})});}:undefined} allowTextExport={adapter.kind==="native"&&current.pages.some(page=>pageAllows(page,"copyContent"))} onExportText={()=>{if(!blocked&&current&&adapter.kind==="native"){updateSearch({open:false});setTextExport({pages:current.pages,currentPageId:current.selectedPageId});}}} allowImageExport={current!.pages.some(page=>pageAllows(page,"copyContent"))} onExportImages={()=>{if(!blocked&&current&&adapter.kind==="native"){setImageExport({pages:current.pages,currentPageId:current.selectedPageId});}}} allowFlatten={canFlatten} allowExtract={canArrange} disabled={blocked} demo={adapter.kind === "demo"} onSave={flatten=>void exportPdf(flatten)} onExtract={openExtract} onSplit={openSplit}/>
      </div>
      <div className="tool-group modes" role="group" aria-label="Editing tools">
        <button className="tool-button" disabled={blocked||!canArrange} onClick={openOrganizer} title="Select, rotate, move, duplicate and extract multiple pages">Organize pages</button>
        {selectedPage?.protection&&!selectedPage.protection.owner&&adapter.unlockSources&&<button className="tool-button" disabled={blocked} onClick={()=>void unlockEditing()} title="Enter the owner password to change this PDF’s permitted actions">Unlock editing</button>}
        <button className={`tool-button ${tool === "select" ? "active" : ""}`} disabled={blocked} onClick={() => chooseTool("select")}><MousePointer2 size={17}/><span>Select</span></button>
        <button ref={paragraphToolButton} aria-pressed={tool==="edit"&&textEditMode==="text"} className={`tool-button ${tool === "edit"&&textEditMode==="text" ? "active" : ""}`} disabled={blocked || !pageAllows(selectedPage,"editContent") || !((adapter.listOccurrenceTextRuns&&adapter.replaceTextOccurrence)||(adapter.listTextRuns && adapter.replaceText) || supportsFormText(adapter))} onClick={() => {setTextEditMode("text");chooseTool("edit");}} title={pageAllows(selectedPage,"editContent")?"Change supported existing PDF text":"This PDF does not permit editing existing text."}><TextCursorInput size={17}/><span>Edit text</span></button>
        <button ref={directParagraphToolButton} aria-pressed={tool==="edit"&&textEditMode==="paragraph"} className={`tool-button ${tool==="edit"&&textEditMode==="paragraph"?"active":""}`} disabled={blocked||!pageAllows(selectedPage,"editContent")||(paragraphEditorKind(adapter)===null&&!supportsFormParagraph(adapter))} onClick={()=>{setTextEditMode("paragraph");chooseTool("edit");}} title="Open an existing paragraph directly on the page">Edit paragraphs</button>
        <button ref={imageToolButton} className={`tool-button ${tool === "images" ? "active" : ""}`} disabled={blocked||!pageAllows(selectedPage,"editContent")||!adapter.listExistingImages||!adapter.previewImageReplacementOccurrence||!adapter.renderPagePlan} onClick={()=>chooseTool("images")} title="Replace a supported existing PDF image">Edit images</button>
        {adapter.createFormField&&<button className="tool-button" disabled={blocked||!pageAllows(selectedPage,"editContent")||!pageAllows(selectedPage,"annotate")} onClick={()=>{
          if(blocked||!activeTab||!current||!selectedPage)return;
          try{formAuthorSelection(current,selectedPage);}catch(error){setFailure(error instanceof Error?error.message:String(error));return;}
          updateSearch({open:false});setFormAuthor({tabId:activeTab.id,page:selectedPage,document:current,adapter,sourceIds:workspace.tabs.flatMap(tab=>tab.sourceIds)});
        }}>Create form field…</button>}
        <button ref={formFillButton} className={`tool-button${formFill?" active":""}`} aria-pressed={!!formFill} disabled={blocked||!pageAllows(selectedPage,"fillForms")||!adapter.listFormFields||!adapter.fillFormField} onClick={beginFormFill}>Fill forms</button>
        {adapter.kind==='native'&&supportsTextRemoval(adapter)&&<button className="tool-button" disabled={blocked||!current.pages.length||current.pages.length>1000||current.pages.some(page=>!!page.protection||!pageAllows(page,'editContent')||!pageAllows(page,'copyContent'))} onClick={()=>{if(!activeTab||blocked)return;try{const request=captureTextRemoval(activeTab);updateSearch({open:false});setTextRemoval(request);}catch(cause){setFailure(errorMessage(cause));}}}>Remove text…</button>}
        {adapter.kind==='native'&&supportsContentRemoval(adapter)&&<button className="tool-button" disabled={blocked||!current.pages.length||current.pages.length>1000||current.pages.some(page=>!!page.protection||!pageAllows(page,'editContent')||!pageAllows(page,'copyContent'))} onClick={()=>{if(!activeTab||blocked)return;try{const request=captureTextRemoval(activeTab,'content');updateSearch({open:false});setTextRemoval(request);}catch(cause){setFailure(errorMessage(cause));}}}>Remove content…</button>}
        <button className="tool-button" disabled={blocked||!pageAllows(selectedPage,"editContent")||!pageAllows(selectedPage,"copyContent")||!adapter.prepareOcrPage||!adapter.applyOcr} onClick={beginOcr} title="Recognize scanned text for search and copying">Recognize text</button>
        <button className={`tool-button ${tool === "text" ? "active" : ""}`} disabled={blocked || !toolAllowed(selectedPage,"text")} title={toolAllowed(selectedPage,"text")?undefined:"This PDF restricts this action. See PDF permissions."} onClick={() => chooseTool("text")}><Type size={17}/><span>Text</span></button>
        <button className={`tool-button ${tool === "area" ? "active" : ""}`} disabled={blocked || !canAnnotate || adapter.kind!=="native"} onClick={() => chooseTool("area")} title="Draw a wrapping text area"><Square size={17}/><span>Text area</span></button>
        <button className={`tool-button ${tool === "image" ? "active" : ""}`} aria-label="Insert image" disabled={blocked||!canAnnotate||adapter.kind!=="native"} onClick={()=>chooseTool("image")} title="Insert a PNG or JPEG on this page">Image</button>
        <button className={`tool-button ${tool === "draw" ? "active" : ""}`} disabled={blocked || !toolAllowed(selectedPage,"draw")} title={toolAllowed(selectedPage,"draw")?undefined:"This PDF restricts this action. See PDF permissions."} onClick={() => chooseTool("draw")}><Pencil size={17}/><span>Draw</span></button>
        <button className={`tool-button ${tool === "signature" ? "active" : ""}`} disabled={blocked || !toolAllowed(selectedPage,"signature")} title={toolAllowed(selectedPage,"signature")?undefined:"This PDF restricts this action. See PDF permissions."} onClick={() => chooseTool("signature")}><PenLine size={17}/><span>Signature</span></button>
        <button className={`tool-button ${tool === "highlight" ? "active" : ""}`} disabled={blocked || !toolAllowed(selectedPage,"highlight")} title={toolAllowed(selectedPage,"highlight")?undefined:"This PDF restricts this action. See PDF permissions."} onClick={() => chooseTool("highlight")}><Highlighter size={17}/><span>Highlight</span></button>
        <button className={`tool-button ${tool === "comment" ? "active" : ""}`} disabled={blocked || !toolAllowed(selectedPage,"comment")} title={toolAllowed(selectedPage,"comment")?undefined:"This PDF restricts this action. See PDF permissions."} onClick={() => chooseTool("comment")}><MessageSquare size={17}/><span>Comment</span></button>
      </div>
    </div>
    <div className={`workspace${textParagraph?" paragraph-editing":""}`} role="tabpanel" aria-labelledby={"tab-" + activeTab!.id}>
      <aside className="sidebar">
        <div className="navigation-switch" role="group" aria-label="Navigation view"><button aria-pressed={navigationView==="pages"} disabled={blocked} onClick={()=>setNavigationView("pages")}>Pages</button><button aria-pressed={navigationView==="bookmarks"} disabled={blocked} onClick={()=>setNavigationView("bookmarks")}>Bookmarks</button></div>
        {navigationView==="bookmarks"?<BookmarkSidebar key={activeTab!.id} document={current} disabled={blocked} readOnly={!canArrange} onNavigate={id=>{updateSearch({open:false});navigateToPage(id);}} onAdd={parentId=>setBookmarkDialog({parentId})} onEdit={id=>setBookmarkDialog({id})} onMove={(id,direction)=>edit(doc=>moveBookmark(doc,id,direction))} onRemove={id=>edit(doc=>removeBookmark(doc,id))} onDiscardError={()=>edit(doc=>({...doc,bookmarkError:undefined,bookmarks:doc.bookmarks??[]}))}/>:<>
        <div className="panel-heading"><span>PAGES</span><em>{current.pages.length}</em></div>
        <PageThumbnails key={activeTab!.id} pages={current.pages} selectedPageId={selectedPage?.id??null} adapter={adapter} blocked={blocked} readingBlocked={readingBlocked} canArrange={canArrange} draggedPageId={draggedPageId} dropTarget={dropTarget}
          onSelect={navigateToPage} onDragStart={(event,pageId)=>{if(blocked||!canArrange){event.preventDefault();return;}event.dataTransfer.setData("text/plain",pageId);event.dataTransfer.effectAllowed="move";setDraggedPageId(pageId);selectPage(pageId);}}
          onDragOver={dragOverPage} onDrop={dropPage} onDragEnd={()=>{setDraggedPageId(null);setDropTarget(null);}}
          onDragLeave={event=>{if(!event.currentTarget.contains(event.relatedTarget as Node|null))setDropTarget(null);}}/>
        <button className="add-pages" onClick={addPdf} disabled={blocked || adapter.kind === "demo"}><Plus size={16}/> Add pages</button>
        </>}
      </aside>
      <div className="document-area">
        {recovery.warning&&<div className="recovery-warning" role="status">{recovery.warning}</div>}
        {searchState.open&&<SearchBar key={"search-"+activeTab!.id} query={searchState.query} onQueryChange={query=>updateSearch({query,index:0})} index={searchState.index} total={search.matches.length} searching={search.searching} error={search.error} hasText={search.hasText} onNext={()=>moveSearch(1)} onPrevious={()=>moveSearch(-1)} onClose={()=>updateSearch({open:false})}/>}
        {tool === "images" && !imageReplacement && <div className="placement-banner">Click an outlined image to replace, move, resize, rotate, or remove it.</div>}
        {tool === "edit" && !textParagraph && <div className="placement-banner"><TextCursorInput size={16}/> {textEditMode==="paragraph"?"Click outlined text to edit its paragraph. Review the selected area before applying.":"Click an outlined text run to edit it. Some fonts and layouts are not supported yet."}</div>}
        {tool === "highlight" && <div className="placement-banner"><Highlighter size={16}/> Drag across selectable text to highlight it. Scanned pages need a text layer.</div>}
        {tool === "comment" && <div className="placement-banner"><MessageSquare size={16}/> Click a page to add a comment.</div>}
        {tool === "area" && <div className="placement-banner"><Square size={16}/> Drag a box for your text, then choose a font. Esc cancels.</div>}
        {pendingSignature && <div className="placement-banner"><PenLine size={16}/> Click a page to place your signature <button aria-label="Cancel signature placement" onClick={() => { setPendingSignature(null); setTool("select"); }}><X size={15}/></button></div>}
        {imagePlacement.pending&&<div className="placement-banner">Click this page to place your image · Enter to place · Esc to cancel <button autoFocus disabled={blocked} onClick={()=>imagePlacement.place()}>Place image</button><button aria-label="Cancel image placement" onClick={()=>{imagePlacement.cancel();setTool("select");}}><X size={15}/></button></div>}
        {paragraphWorkspace}
        <div className="page-nav" role="group" aria-label="Page navigation"><button aria-label="Back to previous view" title="Back to previous view (Alt+Left)" disabled={readingBlocked||!canTravelNavigation(activeTab!.navigationHistory,'back',current.pages.map(page=>page.id))} onClick={()=>travelReading('back')}><ArrowLeft size={16}/></button><button aria-label="Forward to next view" title="Forward to next view (Alt+Right)" disabled={readingBlocked||!canTravelNavigation(activeTab!.navigationHistory,'forward',current.pages.map(page=>page.id))} onClick={()=>travelReading('forward')}><ArrowRight size={16}/></button><i className="page-nav-divider" aria-hidden="true"/><button aria-label="Previous page" disabled={readingBlocked || selectedIndex <= 0} onClick={() => navigateToPage(current.pages[selectedIndex - 1].id)}><ChevronLeft size={16}/></button><button className="page-number-button" aria-label="Go to page" disabled={readingBlocked} onClick={()=>setPageJump(true)}>Page {selectedIndex + 1} of {current.pages.length}</button><button aria-label="Next page" disabled={readingBlocked || selectedIndex >= current.pages.length - 1} onClick={() => navigateToPage(current.pages[selectedIndex + 1].id)}><ChevronRight size={16}/></button></div>
      </div>
      <PropertiesPanel contextKey={formFill?`form:${formFill.page.id}:${activeFormField?.annotationIndex??""}:${formTarget.revision}`:`${activeTab!.id}:${tool}:${selectedOverlay?.id??selectedPage?.id??""}`} autoExpand={!!formFill||!!selectedOverlay||tool==="area"||tool==="draw"}>
        {formFill?(!formOverview&&<FormFillEditor preview={false} canvasAnchor={formAnchor} page={formFill.page} pageNumber={formFill.document.pages.findIndex(page=>page.id===formFill.page.id)+1} adapter={formFill.adapter} selectedAnnotationIndex={formTarget.annotationIndex} selectionRevision={formTarget.revision} onDraftChange={updateFormDirty} onSelectionChange={updateFormSelection} onPosition={formFill.adapter.positionFormField&&pageAllows(formFill.page,"editContent")&&pageAllows(formFill.page,"annotate")?beginFormPosition:undefined} onChoices={formFill.adapter.setFormChoices&&formFill.adapter.inspectFormChoices&&pageAllows(formFill.page,'editContent')&&pageAllows(formFill.page,'annotate')?beginFormChoices:undefined} onRemove={formFill.adapter.removeFormField&&formFill.adapter.inspectFormRemoval&&pageAllows(formFill.page,'editContent')&&pageAllows(formFill.page,'annotate')?beginFormRemoval:undefined} onProperties={formFill.adapter.setFormProperties&&formFill.adapter.inspectFormProperties&&pageAllows(formFill.page,'editContent')&&pageAllows(formFill.page,'annotate')?beginFormProperties:undefined} onTextAppearance={formFill.adapter.setFormTextAppearance&&formFill.adapter.inspectFormTextAppearance&&pageAllows(formFill.page,"editContent")&&pageAllows(formFill.page,"annotate")?beginFormTextAppearance:undefined} onFrameAppearance={formFill.adapter.setFormFrame&&formFill.adapter.inspectFormFrame&&pageAllows(formFill.page,"editContent")&&pageAllows(formFill.page,"annotate")?beginFormFrame:undefined} onApply={applyFormField} onClose={closeForm} onLocate={locateFormField} onOverview={()=>{if(!operation.current&&!formDirtyRef.current)setFormOverview(true);}}/>):<>
        {tool === "area" ? <section className="property-section"><h3>Text area</h3><p>Drag a box on a page, or add one here and adjust its dimensions.</p><button className="button" disabled={blocked || !selectedPage || !defaultPageTextArea(selectedPage)} onClick={()=>{if(selectedPage){const rect=defaultPageTextArea(selectedPage);if(rect)addTextArea(selectedPage.id,rect);}}}>Add text area</button>{selectedPage&&!defaultPageTextArea(selectedPage)?<p>The visible page must be at least 36 pt wide and high to add a text area.</p>:null}<p>Text wraps within the area. If it does not fit, enlarge the area or reduce the text size.</p></section> : tool === "draw" ? <section className="property-section"><h3>Draw</h3>
          <label className="field-label">Pen color<span className="color-input"><input aria-label="Pen color" type="color" value={preferences.penColor} onChange={(event) => setPreferences({ ...preferences, penColor: event.target.value.toUpperCase() })}/><code>{preferences.penColor}</code></span></label>
          <label className="field-label">Pen width · {preferences.penWidth} pt<input aria-label="Pen width" type="range" min="0.5" max="20" step="0.5" value={preferences.penWidth} onChange={(event) => setPreferences({ ...preferences, penWidth: Number(event.target.value) })}/></label>
          <p>Draw directly on any page. Each stroke can be undone. Switch to Select to move or delete a stroke.</p>
        </section> : selectedOverlay && selectedPage ? <OverlayProperties imageAspectLocked={imageAspectLocked} onImageAspectLockChange={setImageAspectLocked} disabled={blocked||!canAnnotate} key={`${activeTab?.id}:${selectedOverlay.id}`} onChooseFont={adapter.kind === "native" ? () => { if (!blocked && canAnnotate && selectedOverlay.type === "text") setFontPicker({tabId: activeTab!.id, pageId: selectedPage.id, overlayId: selectedOverlay.id, text: selectedOverlay.text, fontId: selectedOverlay.fontId, fontSize: selectedOverlay.fontSize, shaping: selectedOverlay.shaping, area: selectedOverlay.area}); } : undefined} pageWidth={selectedPage.width} pageHeight={selectedPage.height} autoEdit={selectedOverlay.id === newTextId} onAutoEdited={() => setNewTextId(null)} overlay={selectedOverlay} onChange={(update) => edit((document) => updateOverlay(document, selectedPage.id, selectedOverlay.id, update))} onDelete={() => edit((document) => removeOverlay(document, selectedPage.id, selectedOverlay.id))}/> : selectedPage && <>
          <section className="property-section"><h3>Page</h3><div className="page-summary"><div className="mini-page" style={{ aspectRatio: `${displaySize!.width}/${displaySize!.height}` }}/><div><strong>Page {selectedIndex + 1}</strong><span>{Math.round(displaySize!.width)} × {Math.round(displaySize!.height)} pt</span><small>{selectedPage.rotation ? `${selectedPage.rotation}° clockwise` : "Original orientation"}</small></div></div></section>
          <section className="property-section"><h3>Arrange</h3><div className="property-grid"><button disabled={blocked || !canArrange || selectedIndex <= 0} onClick={() => edit((document) => movePage(document, selectedPage.id, selectedIndex - 1))}><ArrowUp size={16}/>Move up</button><button disabled={blocked || !canArrange || selectedIndex >= current.pages.length - 1} onClick={() => edit((document) => movePage(document, selectedPage.id, selectedIndex + 1))}><ArrowDown size={16}/>Move down</button><button disabled={blocked || !canArrange} onClick={() => edit((document) => rotatePage(document, selectedPage.id))}><RotateCw size={16}/>Rotate</button><button disabled={blocked || !canArrange} onClick={() => { const id = uniqueId("page"); edit((document) => duplicatePage(document, selectedPage.id, id)); setNavigationRequest((request) => ({ pageId: id, revision: (request?.revision ?? 0) + 1 })); }}><Copy size={16}/>Duplicate</button></div></section>
          <section className="property-section"><h3>Page actions</h3><button className="button" disabled={blocked||!canArrange} onClick={openOrganizer}>Organize pages…</button>{adapter.kind==="native"&&adapter.replacePageContent&&adapter.renderPagePlan&&<button className="button" disabled={blocked||!canReplacePage} onClick={()=>{if(activeTab&&current&&selectedPage&&!blocked){updateSearch({open:false});setPageReplacement({tabId:activeTab.id,page:selectedPage,document:current,adapter,sourceIds:workspace.tabs.flatMap(tab=>tab.sourceIds)});}}}>Replace page content…</button>}{adapter.kind==="native"&&adapter.renderPagePlan&&<button className="button" disabled={blocked||!pageAllows(selectedPage,"editContent")} onClick={()=>{if(activeTab&&current&&selectedPage&&!blocked)setCropRequest({tabId:activeTab.id,page:selectedPage,document:current,adapter});}}>Crop page…</button>}{adapter.createImageDocument&&adapter.kind==="native"&&<button className="button" disabled={blocked||!canArrange} onClick={()=>openImages(true)}>Insert image pages…</button>}{adapter.createBlankDocument&&adapter.kind==="native"&&<button className="button" disabled={blocked||!canArrange} onClick={()=>openBlank(true)}>Insert blank page…</button>}{adapter.kind==="native"&&<button className="button" disabled={blocked||!canArrange} onClick={openExtract}>Extract pages…</button>}{adapter.kind==="native"&&<button className="button" disabled={blocked||!canArrange} onClick={openSplit}>Split PDF…</button>}<button className="danger-action" disabled={blocked || !canArrange || current.pages.length <= 1} onClick={() => edit((document) => deletePage(document, selectedPage.id))}><Trash2 size={16}/>Delete page</button><p>Source files are never changed.</p></section>
        </>}
        {selectedPage?.protection&&<section className="property-section" aria-label="PDF permissions"><h3>PDF permissions</h3><p>{describeRestrictions(selectedPage)}</p>{!selectedPage.protection.owner&&<p>Use Unlock editing with the owner password to change the permitted actions.</p>}</section>}
        <ReviewList nativeNotes={nativeReview.notes} nativeErrors={nativeReview.errors} nativeLoading={nativeReview.loading} onSelectNative={openNativeComment} pages={current.pages} selectedPageId={selectedPage?.id??null} selectedOverlayId={current.selectedOverlayId} disabled={blocked} onSelect={navigateToAnnotation}/>
        </>}
      </PropertiesPanel>
    </div>
    <footer className="statusbar"><span>{busy ?? (imagePlacement.importing?"Opening image…":null) ?? notice ?? (dirty ? "Unsaved edits" : "All changes exported")}</span>{(failure||imagePlacement.error) && <span className="status-error" role="alert">{failure||imagePlacement.error}</span>}<span className="status-spacer"/><span>{selectedPage ? `${Math.round(displaySize!.width)} × ${Math.round(displaySize!.height)} pt` : ""}</span><span>Local only</span></footer>
    {dialogs}
  </div>;
}
