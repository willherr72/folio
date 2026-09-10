import { useEffect, useMemo, useRef, useState } from "react";
import type { FolioAdapter } from "../editor/adapter";
import { clientPointToPage, displayDimensions, pageTransform, placeInkPaths } from "../editor/geometry";
import type { InkPoint, Overlay, PagePlan } from "../editor/types";

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

function usePageImage(adapter: FolioAdapter, page: PagePlan, pixelWidth: number) {
  const key = `${adapter.kind}:${page.sourceId}:${page.pageIndex}:${Math.round(pixelWidth)}`;
  const [state, setState] = useState<{ key: string; url?: string; error?: string }>({ key });
  useEffect(() => {
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
  }, [adapter, key, page.pageIndex, page.sourceId, pixelWidth, page.width, page.height]);
  return state.key === key ? state : { key };
}

function useNearViewport(ref: React.RefObject<HTMLDivElement | null>) {
  const [visible, setVisible] = useState(() => typeof IntersectionObserver === "undefined");
  useEffect(() => {
    if (visible || typeof IntersectionObserver === "undefined" || !ref.current) return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        setVisible(true);
        observer.disconnect();
      }
    }, { rootMargin: "300px 0px" });
    observer.observe(ref.current);
    return () => observer.disconnect();
  }, [ref, visible]);
  return visible;
}
function bounds(overlay: Overlay) {
  if (overlay.type === "text") {
    const lines = overlay.text.split("\n");
    return { x: overlay.x - 4, y: overlay.y - 3, width: Math.max(36, ...lines.map((line) => line.length * overlay.fontSize * .56)) + 8, height: Math.max(overlay.fontSize * 1.2, lines.length * overlay.fontSize * 1.2) + 5 };
  }
  const points = overlay.paths.flat();
  if (!points.length) return { x: 0, y: 0, width: 0, height: 0 };
  const xs = points.map((point) => point.x);
  const ys = points.map((point) => point.y);
  const minX = Math.min(...xs), minY = Math.min(...ys);
  return { x: minX - 5, y: minY - 5, width: Math.max(12, Math.max(...xs) - minX + 10), height: Math.max(12, Math.max(...ys) - minY + 10) };
}

interface PageViewProps {
  adapter: FolioAdapter;
  page: PagePlan;
  pageNumber: number;
  zoom: number;
  tool: "select" | "text" | "signature" | "draw";
  selectedOverlayId: string | null;
  pendingSignature: InkPoint[][] | null;
  onDraw?(path: InkPoint[]): void;
  drawColor?: string;
  drawWidth?: number;
  onActivate?(): void;
  interactionDisabled?: boolean;
  onSelectOverlay(id: string | null): void;
  onAddText(point: InkPoint): void;
  onPlaceSignature(point: InkPoint): void;
  onMoveOverlay(id: string, x: number, y: number, phase: "start" | "move" | "end"): void;
}

export function PageView(props: PageViewProps) {
  const { adapter, page, pageNumber, zoom, tool, selectedOverlayId, pendingSignature } = props;
  const svgRef = useRef<SVGSVGElement>(null);
  const dragRef = useRef<{ id: string; pointer: number; offset: InkPoint } | null>(null);
  const dimensions = displayDimensions(page.width, page.height, page.rotation);
  const strokeRef = useRef<{ pointer: number; path: InkPoint[] } | null>(null);
  const [draft, setDraft] = useState<InkPoint[]>([]);
  const [signaturePoint, setSignaturePoint] = useState<InkPoint | null>(null);
  const [signatureCancelled, setSignatureCancelled] = useState(false);
  const previewEnabled = !!pendingSignature && tool === "signature" && !props.interactionDisabled && !signatureCancelled;
  const previewPaths = previewEnabled && signaturePoint ? placeInkPaths(pendingSignature!, signaturePoint, page.width, page.height) : [];
  const cssWidth = dimensions.width * zoom / 100;
  const rendered = usePageImage(adapter, page, Math.min(2400, Math.max(600, Math.round(cssWidth * devicePixelRatio))));
  const cursor = pendingSignature || tool === "draw" ? "crosshair" : tool === "text" ? "text" : "default";

  const originalPoint = (event: React.PointerEvent) => {
    const rect = svgRef.current!.getBoundingClientRect();
    return clientPointToPage(event.clientX, event.clientY, rect, page.width, page.height, page.rotation);
  };

  const releasePointer = (pointer: number) => {
    // Capture may already have been released by the browser on cancellation.
    try { svgRef.current?.releasePointerCapture(pointer); } catch { /* Already released. */ }
  };

  useEffect(() => {
    const stroke = strokeRef.current;
    if (stroke) releasePointer(stroke.pointer);
    strokeRef.current = null;
    setDraft([]);
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
    if (props.interactionDisabled || event.button !== 0 || strokeRef.current || dragRef.current) return;
    if (tool !== "draw" && event.target !== event.currentTarget && (event.target as Element).closest("[data-overlay]")) return;
    props.onActivate?.();
    const point = originalPoint(event);
    if (tool === "draw") {
      event.preventDefault();
      strokeRef.current = { pointer: event.pointerId, path: [point] };
      setDraft([point]);
      svgRef.current?.setPointerCapture(event.pointerId);
    } else if (previewEnabled) { setSignaturePoint(null); props.onPlaceSignature(point); }
    else if (tool === "text") { event.preventDefault(); props.onAddText(point); }
    else props.onSelectOverlay(null);
  };

  const startDrag = (event: React.PointerEvent, overlay: Overlay) => {
    if (tool === "draw") return;
    event.stopPropagation();
    if (props.interactionDisabled || event.button !== 0 || dragRef.current) return;
    props.onActivate?.();
    props.onSelectOverlay(overlay.id);
    if (tool !== "select") return;
    const value = originalPoint(event);
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
    <div className="page-stage" aria-label={`Page ${pageNumber}`} style={{ width: cssWidth }}>
      <svg ref={svgRef} className="page-canvas" viewBox={`0 0 ${dimensions.width} ${dimensions.height}`} style={{ cursor }}
        onPointerLeave={() => setSignaturePoint(null)} onPointerDown={handleBackground} onPointerMove={moveDrag} onPointerUp={(event) => finishPointer(event)} onPointerCancel={(event) => finishPointer(event, true)} onLostPointerCapture={(event) => finishPointer(event, true)}>
        <g transform={pageTransform(page.width, page.height, page.rotation) || undefined}>
          {rendered.url ? <image href={rendered.url} width={page.width} height={page.height} preserveAspectRatio="none" /> : <rect width={page.width} height={page.height} fill="#fff" />}
          <PdfTextLayer adapter={adapter} page={page} pageNumber={pageNumber} selectable={tool === "select" && !pendingSignature && !props.interactionDisabled} />
          {previewPaths.length > 0 && <g className="signature-preview" aria-hidden="true">{previewPaths.map((path, index) => <polyline key={index} points={path.map((point) => `${point.x},${point.y}`).join(" ")} fill="none" stroke={props.drawColor ?? "#2D2A26"} strokeWidth={props.drawWidth ?? 2} strokeLinecap="round" strokeLinejoin="round" />)}</g>}
          {draft.length > 0 && <polyline className="draft-ink" points={draft.map((point) => `${point.x},${point.y}`).join(" ")} fill="none" stroke={props.drawColor ?? "#2D2A26"} strokeWidth={props.drawWidth ?? 2} strokeLinecap="round" strokeLinejoin="round" pointerEvents="none" />}
          {page.overlays.map((overlay) => {
            const selected = overlay.id === selectedOverlayId;
            const box = bounds(overlay);
            return <g key={overlay.id} data-overlay={overlay.id} className={`overlay ${selected ? "selected" : ""}`} onPointerDown={(event) => startDrag(event, overlay)}>
              {overlay.type === "text" ? (
                <text x={overlay.x} y={overlay.y + overlay.fontSize} fill={overlay.color} xmlSpace="preserve" style={{ whiteSpace: "pre" }} fontFamily="Arial, Helvetica, sans-serif" fontSize={overlay.fontSize}>
                  {overlay.text.split("\n").map((line, index) => <tspan key={index} x={overlay.x} dy={index === 0 ? 0 : overlay.fontSize * 1.2}>{line || " "}</tspan>)}
                </text>
              ) : overlay.paths.map((path, index) => <polyline key={index} points={path.map((point) => `${point.x},${point.y}`).join(" ")} fill="none" stroke={overlay.color} strokeWidth={overlay.strokeWidth} strokeLinecap="round" strokeLinejoin="round" />)}
              {selected && <rect className="selection-box" x={box.x} y={box.y} width={box.width} height={box.height} />}
            </g>;
          })}
        </g>
      </svg>
      {previewPaths.length > 0 && <div className="signature-placement-cue">Click to place · Esc to cancel</div>}
      {!rendered.url && !rendered.error && <div className="page-loading"><span /> Rendering page…</div>}
      {rendered.error && <div className="page-error">Couldn’t render this page<br/><small>{rendered.error}</small></div>}
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
  const dimensions = displayDimensions(page.width, page.height, page.rotation);
  const rendered = usePageImage(adapter, page, 240);
  const transform = useMemo(() => pageTransform(page.width, page.height, page.rotation), [page]);
  return <svg className="thumbnail-canvas" viewBox={`0 0 ${dimensions.width} ${dimensions.height}`}>
    <g transform={transform || undefined}>
      {rendered.url ? <image href={rendered.url} width={page.width} height={page.height} /> : <rect width={page.width} height={page.height} fill="#fff" />}
      {page.overlays.map((overlay) => overlay.type === "text"
        ? <text key={overlay.id} x={overlay.x} y={overlay.y + overlay.fontSize} fontSize={overlay.fontSize} fontFamily="Arial, Helvetica, sans-serif" fill={overlay.color} xmlSpace="preserve" style={{ whiteSpace: "pre" }}>{overlay.text.split("\n")[0]}</text>
        : <g key={overlay.id}>{overlay.paths.map((path, i) => <polyline key={i} points={path.map((point) => `${point.x},${point.y}`).join(" ")} fill="none" stroke={overlay.color} strokeWidth={overlay.strokeWidth} />)}</g>)}
    </g>
  </svg>;
}