import type {CanvasParagraphView} from "../editor/paragraph-canvas";
import {ParagraphCanvasControls} from "./ParagraphCanvasControls";
import type {CanvasImageView} from "../editor/existing-image-canvas";
import {ExistingImageEditingControls} from "./ExistingImageEditingControls";
import {ExistingImageLayer} from "./ExistingImageLayer";
import type {ExistingImage} from "../editor/types";
import {NativeCommentLayer} from "./NativeCommentLayer";
import {ImageHandles,useImageGesture} from "./ImageControls";
import {screenArrowDelta} from "../editor/geometry";
import {moveImage} from "../editor/image-geometry";
import {imageBounds} from "../editor/image-geometry";
import {useComposedPage} from "../editor/use-composed-page";
import {pageAllows,toolAllowed} from "../editor/protection";
import { subscribeShapedText, shapedTextRevision } from "../editor/shaped-text";
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import type { FolioAdapter } from "../editor/adapter";
import { clientPointToPage, displayDimensions, pageVisibleRect, rectVisibleOnPage, pageTransform, placeInkPaths } from "../editor/geometry";
import { textOverlayBounds, textOverlayTransform } from "../editor/text-overlay-geometry";
import { TextAreaHandles, useTextAreaGesture } from "./TextAreaControls";
import { TextOverlayPresentation } from "./TextOverlayPresentation";
import type { SearchMatch } from "../editor/search";
import "./search.css";
import { CommentIcon, HighlightMarks } from "./AnnotationPresentation";
import type { AnnotationRect, EditableTextRun, EditorTool, InkPoint, Overlay, PagePlan } from "../editor/types";
import { ExistingTextLayer } from "./ExistingTextLayer";
import {clearExistingTextCache} from "../editor/existing-text-cache";
import {CanvasFormLayer} from "./CanvasFormLayer";

import { PdfTextLayer, clearPageTextCache } from "./PdfTextLayer";

interface RenderEntry {
  sourceId: string;
  request: Promise<string>;
  references: number;
  disposalRequested: boolean;
  estimatedBytes: number;
}
const renderCache = new Map<string, RenderEntry>();
const MAX_RENDER_CACHE = 60;
// Bound retained decoded image memory; mounted pages stay valid even over budget.
const MAX_RENDER_BYTES = 96 * 1024 * 1024;

function releaseEntry(key: string, entry: RenderEntry) {
  if (renderCache.get(key) !== entry) return;
  renderCache.delete(key);
  void entry.request.then((url) => { if (url.startsWith("blob:")) URL.revokeObjectURL(url); }).catch(() => {});
}

export function clearRenderCache(sourceIds: string[]) {
  clearExistingTextCache(sourceIds);
  clearPageTextCache(sourceIds);
  const targets = new Set(sourceIds);
  for (const [key, entry] of renderCache) {
    if (!targets.has(entry.sourceId)) continue;
    entry.disposalRequested = true;
    if (entry.references === 0) releaseEntry(key, entry);
  }
}

function pruneRenderCache() {
  let bytes = [...renderCache.values()].reduce((sum, entry) => sum + entry.estimatedBytes, 0);
  while (renderCache.size > MAX_RENDER_CACHE || bytes > MAX_RENDER_BYTES) {
    const candidate = [...renderCache.entries()].find(([, entry]) => entry.references === 0);
    if (!candidate) return;
    bytes -= candidate[1].estimatedBytes;
    releaseEntry(candidate[0], candidate[1]);
  }
}

export function usePageImage(adapter: FolioAdapter, page: Pick<PagePlan,"sourceId"|"pageIndex"|"width"|"height">, pixelWidth: number, enabled=true, revision=0) {
  const key = `${adapter.kind}:${page.sourceId}:${page.pageIndex}:${Math.round(pixelWidth)}`;
  const [state, setState] = useState<{ key: string; url?: string; error?: string }>({ key });
  useEffect(() => {
    if(!enabled)return;
    let active = true;
    setState({ key });
    let entry = renderCache.get(key);
    if (!entry) {
      entry = {
        sourceId: page.sourceId,
        request: adapter.renderPage(page.sourceId, page.pageIndex, pixelWidth),
        references: 0,
        disposalRequested: false,
        estimatedBytes: pixelWidth * Math.ceil(pixelWidth * page.height / page.width) * 4,
      };
      renderCache.set(key, entry);
    }
    // Access order makes eviction discard the least recently used idle image.
    renderCache.delete(key);
    renderCache.set(key, entry);
    entry.references += 1;
    pruneRenderCache();
    entry.request.then((url) => active && setState({ key, url })).catch((error: unknown) => {
      if (renderCache.get(key) === entry) renderCache.delete(key);
      if (active) setState({ key, error: error instanceof Error ? error.message : String(error) });
    });
    return () => {
      active = false;
      entry.references = Math.max(0, entry.references - 1);
      if (entry.disposalRequested && entry.references === 0) releaseEntry(key, entry);
      else pruneRenderCache();
    };
  }, [adapter, key, page.pageIndex, page.sourceId, pixelWidth, page.width, page.height,enabled,revision]);
  return enabled&&state.key === key ? state : { key };
}

function useNearViewport(ref: React.RefObject<HTMLDivElement | null>) {
  const [visible, setVisible] = useState(() => typeof IntersectionObserver === "undefined");
  useEffect(() => {
    if (typeof IntersectionObserver === "undefined" || !ref.current) return;
    const observer = new IntersectionObserver((entries) => {
      // Release offscreen thumbnail references so the shared raster cache can evict them.
      const latest = entries[entries.length - 1];
      if (latest) setVisible(latest.isIntersecting);
    }, { rootMargin: "300px 0px" });
    observer.observe(ref.current);
    return () => observer.disconnect();
  }, [ref]);
  return visible;
}
function bounds(overlay: Overlay) {
  if(overlay.type === "image")return imageBounds(overlay);
  if (overlay.type === "text") return textOverlayBounds(overlay);
  if (overlay.type === "comment") return { x: overlay.x, y: overlay.y, width: 20, height: 20 };
  if (overlay.type === "highlight") {
    if (!overlay.rects.length) return { x: 0, y: 0, width: 0, height: 0 };
    const x = Math.min(...overlay.rects.map(rect => rect.x)), y = Math.min(...overlay.rects.map(rect => rect.y));
    return { x, y, width: Math.max(...overlay.rects.map(rect => rect.x + rect.width)) - x, height: Math.max(...overlay.rects.map(rect => rect.y + rect.height)) - y };
  }
  const points = overlay.paths.flat();
  if (!points.length) return { x: 0, y: 0, width: 0, height: 0 };
  const xs = points.map((point) => point.x);
  const ys = points.map((point) => point.y);
  const minX = Math.min(...xs), minY = Math.min(...ys);
  return { x: minX - 5, y: minY - 5, width: Math.max(12, Math.max(...xs) - minX + 10), height: Math.max(12, Math.max(...ys) - minY + 10) };
}

interface PageViewProps {
  canvasImage?:CanvasImageView;
  canvasParagraph?:CanvasParagraphView;
  paragraphEditingActive?:boolean;
  imageEditingActive?:boolean;
  formSelectedAnnotationIndex?:number;
  formDirty?:boolean;
  onFormAnchor?:import('../editor/form-canvas-entry').FormAnchorCallback;
  onSelectPageFormField?(field:import("../editor/types").FormField):void;
  nativeComments?:import("../editor/types").NativeComment[];
  onOpenPageComment?(note:import("../editor/types").NativeComment):void;
  onMovePageComment?(note:import("../editor/types").NativeComment,position:InkPoint):Promise<string|null>;
  imageAspectLocked?:boolean;
  onTransformImage?(before:import("../editor/types").ImageOverlay,after:import("../editor/types").ImageOverlay):void;
  pendingImage?: import("../editor/types").ImageOverlay | null;
  onMoveImage?(point:InkPoint):void;
  onPlaceImage?(point:InkPoint):void;
  searchMatches?: SearchMatch[];
  activeSearchMatchId?: string | null;
  adapter: FolioAdapter;
  page: PagePlan;
  pageNumber: number;
  zoom: number;
  tool: EditorTool;
  onEditText?(run: EditableTextRun): void;
  onEditImage?(image:ExistingImage):void;
  selectedOverlayId: string | null;
  pendingSignature: InkPoint[][] | null;
  onDraw?(path: InkPoint[]): void;
  onHighlight?(rects: AnnotationRect[]): void;
  onAddComment?(point: InkPoint): void;
  drawColor?: string;
  drawWidth?: number;
  onActivate?(): void;
  interactionDisabled?: boolean;
  onSelectOverlay(id: string | null): void;
  onAddText(point: InkPoint): void;
  onAddTextArea?(rect: AnnotationRect): void;
  onResizeTextArea?(id: string, rect: AnnotationRect): void;
  onPlaceSignature(point: InkPoint): void;
  onMoveOverlay(id: string, x: number, y: number, phase: "start" | "move" | "end"): void;
}

export function PageView(props: PageViewProps) {
  const { adapter, page, pageNumber, zoom, tool:requestedTool, selectedOverlayId, pendingSignature } = props;
  const tool=toolAllowed(page,requestedTool)?requestedTool:"select";
  const canAnnotate=pageAllows(page,"annotate");
  const svgRef = useRef<SVGSVGElement>(null);
  const [nativeStatus,setNativeStatus]=useState<{text:string;error?:boolean}|null>(null);
  const imageGesture=useImageGesture({page,tool,zoom,selectedId:selectedOverlayId,disabled:!!props.interactionDisabled||!canAnnotate,locked:props.imageAspectLocked!==false,svg:svgRef,onChange:props.onTransformImage});
  const areaGesture = useTextAreaGesture({page,tool,zoom,selectedId:selectedOverlayId,disabled:props.interactionDisabled||!canAnnotate,svg:svgRef,onAdd:props.onAddTextArea,onResize:props.onResizeTextArea});
  const overlays = useMemo(()=>page.overlays.map(overlay => imageGesture.draft?.id===overlay.id?imageGesture.draft:areaGesture.draft?.overlay?.id === overlay.id ? areaGesture.draft.overlay : overlay),[page.overlays,areaGesture.draft?.overlay,imageGesture.draft]);
  const selectedImage=overlays.find(overlay=>overlay.id===selectedOverlayId&&overlay.type==="image"&&(!page.crop||rectVisibleOnPage(bounds(overlay),page)));
  const selectedArea = overlays.find(overlay => overlay.id === selectedOverlayId && overlay.type === "text" && overlay.area&&(!page.crop||rectVisibleOnPage(bounds(overlay),page)));
  const dragRef = useRef<{ id: string; pointer: number; offset: InkPoint } | null>(null);
  const dimensions = pageVisibleRect(page);
  const fullDimensions = displayDimensions(page.width,page.height,page.rotation);
  const strokeRef = useRef<{ pointer: number; path: InkPoint[] } | null>(null);
  const [draft, setDraft] = useState<InkPoint[]>([]);
  const [signaturePoint, setSignaturePoint] = useState<InkPoint | null>(null);
  const [signatureCancelled, setSignatureCancelled] = useState(false);
  const previewEnabled = !!pendingSignature && tool === "signature" && !props.interactionDisabled && !signatureCancelled;
  const previewPaths = useMemo(()=>previewEnabled && signaturePoint ? placeInkPaths(pendingSignature!, signaturePoint, page.width, page.height) : [],[previewEnabled,signaturePoint,pendingSignature,page.width,page.height]);
  const cssWidth = dimensions.width * zoom / 100;
  const pendingImage=tool==="image"&&!props.interactionDisabled?props.pendingImage:null;
  const nativeRotation=page.rotation!==0&&adapter.kind==="native"&&!!adapter.renderPagePlan;
  const rasterDimensions=nativeRotation&&!props.canvasParagraph?fullDimensions:{width:page.width,height:page.height};
  const rasterTransform=nativeRotation&&!props.canvasParagraph?pageTransform(fullDimensions.width,fullDimensions.height,((360-page.rotation)%360) as import("../editor/types").Rotation):undefined;
  const composite=!!props.canvasParagraph||!!props.canvasImage||nativeRotation||!!pendingImage||page.overlays.some(overlay=>overlay.type==="image");
  const interactionOverlays=composite
    ? [...overlays.filter(overlay=>overlay.type!=="highlight"&&overlay.type!=="comment"),...overlays.filter(overlay=>overlay.type==="highlight"||overlay.type==="comment")]
    : overlays;
  const pixelWidth=Math.min(2400, Math.max(600, Math.round(fullDimensions.width * zoom / 100 * devicePixelRatio)));
  const original=usePageImage(adapter,page,pixelWidth,!composite);
  const composedOverlays=useMemo(()=>{
    if(!composite||(!pendingImage&&!previewPaths.length&&draft.length<2))return overlays;
    const next=pendingImage?[...overlays,pendingImage]:[...overlays];
    for(const paths of [previewPaths,draft.length>1?[draft]:[]])if(paths.length){
      let id="folio-preview-ink";while(next.some(overlay=>overlay.id===id))id+="-";
      next.push({type:"ink",id,paths,color:props.drawColor??"#2D2A26",strokeWidth:props.drawWidth??2});
    }
    return next;
  },[composite,overlays,pendingImage,previewPaths,draft,props.drawColor,props.drawWidth]);
  // The raster keeps the full source frame; the SVG viewBox performs cropping.
  const composed=useComposedPage(adapter,{...page,crop:undefined,overlays:composedOverlays},pixelWidth,composite&&!props.canvasImage&&!props.canvasParagraph,nativeRotation);
  const rendered=props.canvasParagraph?.raster??props.canvasImage?.raster??(composite?composed:original);
  useSyncExternalStore(subscribeShapedText, shapedTextRevision);
  const cursor = pendingSignature || tool === "image" || tool === "draw" || tool === "comment" || tool === "area" ? "crosshair" : tool === "text" || tool === "highlight" ? "text" : "default";

  const originalPoint = (event: React.PointerEvent) => {
    const rect = svgRef.current!.getBoundingClientRect();
    return clientPointToPage(event.clientX, event.clientY, rect, page.width, page.height, page.rotation,page.crop);
  };

  const releasePointer = (pointer: number) => {
    // Capture may already have been released by the browser on cancellation.
    try { svgRef.current?.releasePointerCapture(pointer); } catch { /* Already released. */ }
  };

  useEffect(() => {
    const stroke = strokeRef.current;
    if (stroke) releasePointer(stroke.pointer);
    strokeRef.current = null;
    setDraft(value=>value.length?[]:value);
    const drag = dragRef.current;
    if (drag) {
      dragRef.current = null;
      releasePointer(drag.pointer);
      props.onMoveOverlay(drag.id, 0, 0, "end");
    }
  }, [page.id, page.rotation, tool, props.interactionDisabled]);

  useEffect(() => {
    setSignaturePoint(null);
    setSignatureCancelled(false);
  }, [page.id, page.rotation, tool, pendingSignature, props.interactionDisabled]);

  useEffect(() => {
    if (!pendingSignature) return;
    const cancel = (event: KeyboardEvent) => {
      if (event.key === "Escape") { setSignaturePoint(null); setSignatureCancelled(true); }
    };
    window.addEventListener("keydown", cancel);
    return () => window.removeEventListener("keydown", cancel);
  }, [pendingSignature]);

  const handleBackground = (event: React.PointerEvent<SVGSVGElement>) => {
    if (props.interactionDisabled || event.button !== 0 || strokeRef.current || dragRef.current || areaGesture.active || imageGesture.isActive()) return;
    if(tool==="image"){event.preventDefault();if(pendingImage&&!rendered.error)props.onPlaceImage?.(originalPoint(event));return;}
    if (tool === "edit" || tool === "form" || tool === "images") { props.onActivate?.(); return; }
    if (tool !== "draw" && tool !== "area" && event.target !== event.currentTarget && (event.target as Element).closest("[data-overlay]")) return;
    props.onActivate?.();
    const point = originalPoint(event);
    if (tool === "area") { areaGesture.beginCreate(event,point); }
    else if (tool === "draw") {
      event.preventDefault();
      strokeRef.current = { pointer: event.pointerId, path: [point] };
      setDraft([point]);
      svgRef.current?.setPointerCapture(event.pointerId);
    } else if (previewEnabled) { setSignaturePoint(null); props.onPlaceSignature(point); }
    else if (tool === "text") { event.preventDefault(); props.onAddText(point); }
    else if (tool === "comment") { event.preventDefault(); props.onAddComment?.(point); }
    else {
      const highlight = tool === "select" ? [...page.overlays].reverse().find(overlay => overlay.type === "highlight" && overlay.rects.some(rect => point.x >= rect.x && point.x <= rect.x + rect.width && point.y >= rect.y && point.y <= rect.y + rect.height)) : undefined;
      props.onSelectOverlay(highlight?.id ?? null);
    }
  };

  const startDrag = (event: React.PointerEvent, overlay: Overlay) => {
    if (tool === "edit" || tool === "form" || tool === "images") { event.stopPropagation(); return; }
    if (tool === "draw" || tool === "area" || tool === "image") return;
    event.stopPropagation();
    if (props.interactionDisabled || event.button !== 0 || dragRef.current || areaGesture.isActive() || imageGesture.isActive()) return;
    props.onActivate?.();
    props.onSelectOverlay(overlay.id);
    if (!canAnnotate || tool !== "select" || overlay.type === "highlight") return;
    const value = originalPoint(event);
    if(overlay.type==="image"&&props.onTransformImage){imageGesture.begin(event,overlay,value);return;}
    const box = bounds(overlay);
    dragRef.current = { id: overlay.id, pointer: event.pointerId, offset: { x: value.x - box.x, y: value.y - box.y } };
    svgRef.current?.setPointerCapture(event.pointerId);
    props.onMoveOverlay(overlay.id, box.x, box.y, "start");
  };

  const appendPoint = (point: InkPoint) => {
    const stroke = strokeRef.current;
    if (!stroke) return;
    const last = stroke.path[stroke.path.length - 1];
    if (point.x !== last.x || point.y !== last.y) stroke.path.push(point);
  };

  const moveDrag = (event: React.PointerEvent<SVGSVGElement>) => {
    if (props.interactionDisabled) return;
    if(pendingImage){props.onMoveImage?.(originalPoint(event));return;}
    if(imageGesture.move(event,originalPoint(event)))return;
    if (areaGesture.move(event,originalPoint(event))) return;
    if (previewEnabled) setSignaturePoint(originalPoint(event));
    const stroke = strokeRef.current;
    if (stroke?.pointer === event.pointerId) {
      const samples = event.nativeEvent.getCoalescedEvents?.() ?? [];
      for (const sample of samples) appendPoint(originalPoint(sample as unknown as React.PointerEvent));
      appendPoint(originalPoint(event));
      setDraft([...stroke.path]);
      return;
    }
    const drag = dragRef.current;
    if (!drag || drag.pointer !== event.pointerId) return;
    const value = originalPoint(event);
    props.onMoveOverlay(drag.id, value.x - drag.offset.x, value.y - drag.offset.y, "move");
  };

  const finishPointer = (event: React.PointerEvent<SVGSVGElement>, cancelled = false) => {
    if(imageGesture.finish(event,cancelled?null:originalPoint(event),cancelled))return;
    if (areaGesture.finish(event,cancelled ? null : originalPoint(event),cancelled)) return;
    const stroke = strokeRef.current;
    if (stroke?.pointer === event.pointerId) {
      if (!cancelled && !props.interactionDisabled) appendPoint(originalPoint(event));
      strokeRef.current = null;
      setDraft([]);
      releasePointer(event.pointerId);
      if (!cancelled && !props.interactionDisabled && stroke.path.length > 1) props.onDraw?.(stroke.path);
    }
    const drag = dragRef.current;
    if (!drag || drag.pointer !== event.pointerId) return;
    dragRef.current = null;
    releasePointer(event.pointerId);
    props.onMoveOverlay(drag.id, 0, 0, "end");
  };
  return (
    <div className="page-stage" aria-label={`Page ${pageNumber}`} aria-busy={props.canvasParagraph?props.canvasParagraph.raster.loading:props.canvasImage?props.canvasImage.raster.loading:composite?composed.loading:!original.url&&!original.error} style={{ width: cssWidth }}>
      <svg ref={svgRef} className="page-canvas" viewBox={`${dimensions.x} ${dimensions.y} ${dimensions.width} ${dimensions.height}`} style={{ cursor }}
        onPointerLeave={() => setSignaturePoint(null)} onPointerDown={handleBackground} onPointerMove={moveDrag} onPointerUp={(event) => finishPointer(event)} onPointerCancel={(event) => finishPointer(event, true)} onLostPointerCapture={(event) => finishPointer(event, true)}>
        <g transform={pageTransform(page.width, page.height, page.rotation) || undefined}>
          {!rendered.url && <rect width={page.width} height={page.height} fill="#fff" />}
          <image key="page-raster" href={rendered.url} width={rasterDimensions.width} height={rasterDimensions.height} transform={rasterTransform} preserveAspectRatio="none" />
          {!composite && page.overlays.map(overlay => overlay.type === "highlight" ? <HighlightMarks key={overlay.id} overlay={overlay} selected={overlay.id === selectedOverlayId} /> : null)}
          {!!props.searchMatches?.length && <g className="search-highlights" aria-hidden="true">{props.searchMatches.map((match) => match.rects.map((rect, index) =>
            <rect key={`${match.id}:${index}`} {...rect} className={`search-highlight${match.id === props.activeSearchMatchId ? " active" : ""}`} />))}</g>}
          <PdfTextLayer adapter={adapter} page={page} pageNumber={pageNumber} selectable={(tool === "select" || tool === "highlight") && !pendingSignature && !props.interactionDisabled} highlighting={tool === "highlight"} onHighlight={props.onHighlight} />
          {!composite && previewPaths.length > 0 && <g className="signature-preview" aria-hidden="true">{previewPaths.map((path, index) => <polyline key={index} points={path.map((point) => `${point.x},${point.y}`).join(" ")} fill="none" stroke={props.drawColor ?? "#2D2A26"} strokeWidth={props.drawWidth ?? 2} strokeLinecap="round" strokeLinejoin="round" />)}</g>}
          {!composite && draft.length > 0 && <polyline className="draft-ink" points={draft.map((point) => `${point.x},${point.y}`).join(" ")} fill="none" stroke={props.drawColor ?? "#2D2A26"} strokeWidth={props.drawWidth ?? 2} strokeLinecap="round" strokeLinejoin="round" pointerEvents="none" />}
          <NativeCommentLayer nativeRotation={nativeRotation} page={page} notes={props.nativeComments??[]} zoom={zoom} enabled={tool==="select"&&!pendingSignature&&!pendingImage} disabled={!!props.interactionDisabled||!rendered.url}
            onSelect={()=>{props.onActivate?.();props.onSelectOverlay(null);}} onOpen={note=>props.onOpenPageComment?.(note)} onMove={props.onMovePageComment} onStatus={setNativeStatus}/>
          {interactionOverlays.map((overlay) => {
            if(page.crop&&!rectVisibleOnPage(bounds(overlay),page))return null;
            if (overlay.type === "highlight" && !composite) return null;
            const selected = overlay.id === selectedOverlayId;
            const box = bounds(overlay);
            return <g key={overlay.id} data-overlay={overlay.id} className={`overlay ${selected ? "selected" : ""}`} role="button" tabIndex={props.interactionDisabled || tool === "edit" || tool === "form" || tool === "images" ? -1 : 0} aria-label={overlay.type === "image" ? "Image" : overlay.type === "highlight" ? "Highlight" : overlay.type === "comment" ? `Comment: ${overlay.text || "Empty comment"}` : overlay.type === "text" ? `Text: ${overlay.text}` : "Ink annotation"} onKeyDown={(event) => {
                if(overlay.type==="image"&&tool==="select"&&canAnnotate&&!props.interactionDisabled&&!imageGesture.isActive()&&!event.ctrlKey&&!event.metaKey&&!event.altKey){
                  const delta=screenArrowDelta(event.key,event.shiftKey,page.rotation);
                  if(delta){event.preventDefault();event.stopPropagation();const next=moveImage(overlay,overlay.x+delta.x,overlay.y+delta.y,page.width,page.height);if(next!==overlay)props.onTransformImage?.(overlay,next);return;}
                }
                if (!props.interactionDisabled && tool !== "edit" && tool !== "form" && tool !== "images" && tool !== "image" && (event.key === "Enter" || event.key === " ")) { event.preventDefault(); props.onActivate?.(); props.onSelectOverlay(overlay.id); } }} onPointerDown={(event) => startDrag(event, overlay)}>
              {composite && (overlay.type === "highlight" ? overlay.rects.map((rect,index)=><rect key={index} {...rect} fill="transparent" pointerEvents="all"/>) : <rect {...box} fill="transparent" pointerEvents="all"/>)}
              {overlay.type === "text" && overlay.area && <rect className="text-area-hit" x={overlay.x} y={overlay.y} width={overlay.area.width} height={overlay.area.height} transform={textOverlayTransform(overlay)}/>}
              {!composite && (overlay.type === "text" ? <TextOverlayPresentation overlay={overlay} /> : overlay.type === "comment" ? <CommentIcon overlay={overlay} /> : overlay.type === "ink" ? overlay.paths.map((path, index) => <polyline key={index} points={path.map((point) => `${point.x},${point.y}`).join(" ")} fill="none" stroke={overlay.color} strokeWidth={overlay.strokeWidth} strokeLinecap="round" strokeLinejoin="round" />) : null)}
              {overlay.type === "text" && overlay.area ? (selected || !overlay.text) && <rect className={selected ? "selection-box" : "text-area-empty"} x={overlay.x} y={overlay.y} width={overlay.area.width} height={overlay.area.height} transform={textOverlayTransform(overlay)}/> : selected && <rect className="selection-box" x={box.x} y={box.y} width={box.width} height={box.height} />}
            </g>;
          })}
          {pendingImage&&<rect className="selection-box" {...imageBounds(pendingImage)} pointerEvents="none"/>}
          {selectedImage?.type==="image"&&tool==="select"&&canAnnotate&&!props.interactionDisabled&&<ImageHandles image={selectedImage} page={page} zoom={zoom} locked={props.imageAspectLocked!==false}
            onStart={(event,image,corner)=>{event.stopPropagation();if(!dragRef.current&&!areaGesture.isActive())imageGesture.begin(event,image,originalPoint(event),corner);}}
            onChange={(before,after)=>{if(!dragRef.current&&!areaGesture.isActive()&&!imageGesture.isActive())props.onTransformImage?.(before,after);}}/>}
          {areaGesture.draft?.rect && <rect className="text-area-draft" {...areaGesture.draft.rect}/>}
          {selectedArea?.type === "text" && selectedArea.area && tool === "select" && canAnnotate && !props.interactionDisabled && <TextAreaHandles overlay={selectedArea} page={page} zoom={zoom}
            onStart={(event,overlay,corner)=>{event.stopPropagation();if(!dragRef.current)areaGesture.beginResize(event,overlay,corner);}}
            onResize={(id,rect)=>{if(!dragRef.current&&!areaGesture.isActive())props.onResizeTextArea?.(id,rect);}}/>}
          {props.canvasParagraph&&<ParagraphCanvasControls view={props.canvasParagraph} page={page} zoom={zoom}/>}
          {props.canvasImage&&<ExistingImageEditingControls view={props.canvasImage} page={page} zoom={zoom}/>}
          {tool === "images" && !props.imageEditingActive && <ExistingImageLayer adapter={adapter} page={page} disabled={props.interactionDisabled} onEditImage={image=>{props.onActivate?.();props.onEditImage?.(image);}}/>}
          {tool === "edit" && !props.paragraphEditingActive && <ExistingTextLayer adapter={adapter} page={page} disabled={props.interactionDisabled} onEditText={run => { props.onActivate?.(); props.onEditText?.(run); }} />}
        </g>
        {tool==="form"&&<CanvasFormLayer adapter={adapter} page={page} selectedAnnotationIndex={props.formSelectedAnnotationIndex} onAnchor={props.onFormAnchor} disabled={!!props.interactionDisabled||!rendered.url} dirty={!!props.formDirty} onSelect={field=>props.onSelectPageFormField?.(field)}/>}
      </svg>
      {nativeStatus&&<div className={`native-comment-cue${nativeStatus.error?" error":""}`} role={nativeStatus.error?"alert":"status"}>{nativeStatus.text}</div>}
      {previewPaths.length > 0 && <div className="signature-placement-cue">Click to place · Esc to cancel</div>}
      {!rendered.url && !rendered.error && <div className="page-loading"><span /> Rendering page…</div>}
      {rendered.error && <div className="page-error" role="alert">Couldn’t render this page<br/><small>{rendered.error}</small>{composite&&<button className="button" onClick={props.canvasParagraph?.raster.retry??props.canvasImage?.raster.retry??composed.retry}>Retry preview</button>}</div>}
    </div>
  );
}

export function Thumbnail({ adapter, page }: { adapter: FolioAdapter; page: PagePlan }) {
  const hostRef = useRef<HTMLDivElement>(null);
  const visible = useNearViewport(hostRef);
  return <div className="thumbnail-render" ref={hostRef}>
    {visible ? <ThumbnailImage adapter={adapter} page={page} /> : <div className="thumbnail-placeholder" />}
  </div>;
}

function ThumbnailImage({ adapter, page }: { adapter: FolioAdapter; page: PagePlan }) {
  const dimensions = pageVisibleRect(page);
  const fullDimensions = displayDimensions(page.width,page.height,page.rotation);
  const nativeRotation=page.rotation!==0&&adapter.kind==="native"&&!!adapter.renderPagePlan;
  const rasterDimensions=nativeRotation?fullDimensions:{width:page.width,height:page.height};
  const rasterTransform=nativeRotation?pageTransform(fullDimensions.width,fullDimensions.height,((360-page.rotation)%360) as import("../editor/types").Rotation):undefined;
  const composite=nativeRotation||page.overlays.some(overlay=>overlay.type==="image");
  const original=usePageImage(adapter,page,240,!composite);
  const composed=useComposedPage(adapter,{...page,crop:undefined},240,composite,nativeRotation);
  const rendered=composite?composed:original;
  const transform = useMemo(() => pageTransform(page.width, page.height, page.rotation), [page]);
  return <svg className="thumbnail-canvas" viewBox={`${dimensions.x} ${dimensions.y} ${dimensions.width} ${dimensions.height}`}>
    <g transform={transform || undefined}>
      {rendered.url ? <image href={rendered.url} width={rasterDimensions.width} height={rasterDimensions.height} transform={rasterTransform} /> : <rect width={page.width} height={page.height} fill="#fff" />}
      {!composite && page.overlays.map((overlay) => overlay.type === "text"
        ? <TextOverlayPresentation key={overlay.id} overlay={overlay} />
        : overlay.type === "highlight" ? <HighlightMarks key={overlay.id} overlay={overlay} />
        : overlay.type === "comment" ? <CommentIcon key={overlay.id} overlay={overlay} />
        : overlay.type === "ink" ? <g key={overlay.id}>{overlay.paths.map((path, i) => <polyline key={i} points={path.map((point) => `${point.x},${point.y}`).join(" ")} fill="none" stroke={overlay.color} strokeWidth={overlay.strokeWidth} />)}</g> : null)}
    </g>
  </svg>;
}
