import { useEffect, useMemo, useRef, useState } from "react";
import type { FolioAdapter } from "../editor/adapter";
import { clientPointToPage, displayDimensions, pageTransform } from "../editor/geometry";
import type { InkPoint, Overlay, PagePlan } from "../editor/types";

interface RenderEntry {
  sourceId: string;
  request: Promise<string>;
  references: number;
  disposalRequested: boolean;
}
const renderCache = new Map<string, RenderEntry>();
const MAX_RENDER_CACHE = 60;

function releaseEntry(key: string, entry: RenderEntry) {
  if (renderCache.get(key) !== entry) return;
  renderCache.delete(key);
  void entry.request.then((url) => { if (url.startsWith("blob:")) URL.revokeObjectURL(url); }).catch(() => {});
}

export function clearRenderCache(sourceIds: string[]) {
  const targets = new Set(sourceIds);
  for (const [key, entry] of renderCache) {
    if (!targets.has(entry.sourceId)) continue;
    entry.disposalRequested = true;
    if (entry.references === 0) releaseEntry(key, entry);
  }
}

function pruneRenderCache() {
  while (renderCache.size > MAX_RENDER_CACHE) {
    const candidate = [...renderCache.entries()].find(([, entry]) => entry.references === 0);
    if (!candidate) return;
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
      };
      renderCache.set(key, entry);
    }
    entry.references += 1;
    pruneRenderCache();
    entry.request.then((url) => active && setState({ key, url })).catch((error: unknown) => {
      if (renderCache.get(key) === entry) renderCache.delete(key);
      if (active) setState({ key, error: error instanceof Error ? error.message : String(error) });
    });
    return () => {
      active = false;
      entry.references = Math.max(0, entry.references - 1);
      if (entry.disposalRequested) releaseEntry(key, entry);
      else pruneRenderCache();
    };
  }, [adapter, key, page.pageIndex, page.sourceId, pixelWidth]);
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
  tool: "select" | "text" | "signature";
  selectedOverlayId: string | null;
  pendingSignature: InkPoint[][] | null;
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
  const cssWidth = dimensions.width * zoom / 100;
  const rendered = usePageImage(adapter, page, Math.min(2400, Math.max(600, Math.round(cssWidth * devicePixelRatio))));
  const cursor = pendingSignature ? "crosshair" : tool === "text" ? "text" : "default";

  const originalPoint = (event: React.PointerEvent) => {
    const rect = svgRef.current!.getBoundingClientRect();
    return clientPointToPage(event.clientX, event.clientY, rect, page.width, page.height, page.rotation);
  };

  const handleBackground = (event: React.PointerEvent<SVGSVGElement>) => {
    if (event.target !== event.currentTarget && (event.target as Element).closest("[data-overlay]")) return;
    const point = originalPoint(event);
    if (pendingSignature) props.onPlaceSignature(point);
    else if (tool === "text") props.onAddText(point);
    else props.onSelectOverlay(null);
  };

  const startDrag = (event: React.PointerEvent, overlay: Overlay) => {
    event.stopPropagation();
    props.onSelectOverlay(overlay.id);
    if (tool !== "select") return;
    const value = originalPoint(event);
    const box = bounds(overlay);
    dragRef.current = { id: overlay.id, pointer: event.pointerId, offset: { x: value.x - box.x, y: value.y - box.y } };
    svgRef.current?.setPointerCapture(event.pointerId);
    props.onMoveOverlay(overlay.id, box.x, box.y, "start");
  };

  const moveDrag = (event: React.PointerEvent<SVGSVGElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointer !== event.pointerId) return;
    const value = originalPoint(event);
    props.onMoveOverlay(drag.id, value.x - drag.offset.x, value.y - drag.offset.y, "move");
  };

  const endDrag = (event: React.PointerEvent<SVGSVGElement>) => {
    const drag = dragRef.current;
    if (!drag || drag.pointer !== event.pointerId) return;
    dragRef.current = null;
    props.onMoveOverlay(drag.id, 0, 0, "end");
  };

  return (
    <div className="page-stage" aria-label={`Page ${pageNumber}`} style={{ width: cssWidth }}>
      <svg ref={svgRef} className="page-canvas" viewBox={`0 0 ${dimensions.width} ${dimensions.height}`} style={{ cursor }}
        onPointerDown={handleBackground} onPointerMove={moveDrag} onPointerUp={endDrag} onPointerCancel={endDrag}>
        <g transform={pageTransform(page.width, page.height, page.rotation) || undefined}>
          {rendered.url ? <image href={rendered.url} width={page.width} height={page.height} preserveAspectRatio="none" /> : <rect width={page.width} height={page.height} fill="#fff" />}
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