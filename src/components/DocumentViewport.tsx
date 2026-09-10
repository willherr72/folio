import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { FolioAdapter } from "../editor/adapter";
import { displayDimensions } from "../editor/geometry";
import type { InkPoint, PagePlan } from "../editor/types";
import { PageView } from "./PageView";

export interface DocumentViewportProps {
  initialScrollPosition?: { top: number; left: number };
  onScrollPositionChange?(position: { top: number; left: number }): void;
  adapter: FolioAdapter;
  pages: PagePlan[];
  selectedPageId: string | null;
  selectedOverlayId: string | null;
  zoom: number;
  onZoomChange(zoom: number): void;
  viewMode: "continuous" | "single";
  tool: "select" | "text" | "signature" | "draw";
  penColor: string;
  penWidth: number;
  pendingSignature: InkPoint[][] | null;
  interactionDisabled: boolean;
  navigationRequest: { pageId: string; revision: number } | null;
  onSelectPage(id: string): void;
  onSelectOverlay(pageId: string, id: string | null): void;
  onAddText(pageId: string, point: InkPoint): void;
  onPlaceSignature(pageId: string, point: InkPoint): void;
  onMoveOverlay(pageId: string, id: string, x: number, y: number, phase: "start" | "move" | "end"): void;
  onDraw(pageId: string, path: InkPoint[]): void;
}

interface ZoomAnchor { pageId: string; x: number; y: number; clientX: number; clientY: number }

export function DocumentViewport(props: DocumentViewportProps) {
  const hostRef = useRef<HTMLElement>(null);
  const slots = useRef(new Map<string, HTMLDivElement>());
  const latest = useRef(props);
  latest.current = props;
  const [nearby, setNearby] = useState<Set<string>>(() => new Set([props.selectedPageId ?? props.pages[0]?.id]));
  const [activePage, setActivePage] = useState<string | null>(null);
  const zoomAnchor = useRef<ZoomAnchor | null>(null);
  const renderedPages = props.viewMode === "single"
    ? props.pages.filter((page) => page.id === (props.selectedPageId ?? props.pages[0]?.id))
    : props.pages;

  const measure = useCallback((selectPage = false) => {
    const host = hostRef.current;
    if (!host) return;
    const viewport = host.getBoundingClientRect();
    const next = new Set<string>();
    let bestPage: string | null = null;
    let bestVisible = 0;
    for (const [id, element] of slots.current) {
      const rect = element.getBoundingClientRect();
      if (rect.bottom >= viewport.top - 500 && rect.top <= viewport.bottom + 500) next.add(id);
      const visible = Math.max(0, Math.min(rect.bottom, viewport.bottom) - Math.max(rect.top, viewport.top));
      if (visible > bestVisible) { bestVisible = visible; bestPage = id; }
    }
    setNearby((previous) => previous.size === next.size && [...next].every((id) => previous.has(id)) ? previous : next);
    const current = latest.current;
    if (selectPage && !current.interactionDisabled && current.viewMode === "continuous" && bestPage && bestPage !== current.selectedPageId) current.onSelectPage(bestPage);
  }, []);

  useLayoutEffect(() => {
    const anchor = zoomAnchor.current;
    const host = hostRef.current;
    if (anchor && host) {
      const slot = slots.current.get(anchor.pageId);
      if (slot) {
        const rect = slot.getBoundingClientRect();
        host.scrollLeft += rect.left + anchor.x * rect.width - anchor.clientX;
        host.scrollTop += rect.top + anchor.y * rect.height - anchor.clientY;
      }
      zoomAnchor.current = null;
    }
    measure();
  }, [props.zoom, props.pages, props.viewMode, measure]);

  const lastNavigation = useRef<string | null>(props.initialScrollPosition && props.navigationRequest ? props.navigationRequest.pageId + ":" + props.navigationRequest.revision : null);
  useLayoutEffect(() => {
    const host = hostRef.current;
    if (host && props.initialScrollPosition) {
      host.scrollTop = props.initialScrollPosition.top;
      host.scrollLeft = props.initialScrollPosition.left;
      measure();
    }
  }, []);
  const previousMode = useRef(props.viewMode);
  useLayoutEffect(() => {
    const request = props.navigationRequest;
    const token = request ? `${request.pageId}:${request.revision}` : null;
    const modeChanged = previousMode.current !== props.viewMode;
    previousMode.current = props.viewMode;
    if (!modeChanged && (!request || token === lastNavigation.current)) return;
    const targetId = modeChanged ? props.selectedPageId : request?.pageId;
    const target = slots.current.get(targetId ?? "");
    const host = hostRef.current;
    if (!target || !host) return;
    lastNavigation.current = token;
    const rect = target.getBoundingClientRect();
    const viewport = host.getBoundingClientRect();
    host.scrollTop += rect.top - viewport.top - 38;
    measure();
  }, [props.navigationRequest, props.selectedPageId, props.pages, props.viewMode, measure]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    const onWheel = (event: WheelEvent) => {
      if (!event.ctrlKey) return;
      event.preventDefault();
      const current = latest.current;
      if (current.interactionDisabled || event.deltaY === 0) return;
      const nextZoom = Math.max(50, Math.min(200, current.zoom + (event.deltaY < 0 ? 10 : -10)));
      if (nextZoom === current.zoom) return;
      let nearest: { id: string; rect: DOMRect; distance: number } | null = null;
      for (const [id, element] of slots.current) {
        const rect = element.getBoundingClientRect();
        const distance = Math.max(rect.top - event.clientY, event.clientY - rect.bottom, 0);
        if (!nearest || distance < nearest.distance) nearest = { id, rect, distance };
      }
      if (nearest && nearest.rect.width > 0 && nearest.rect.height > 0) {
        zoomAnchor.current = { pageId: nearest.id, x: (event.clientX - nearest.rect.left) / nearest.rect.width, y: (event.clientY - nearest.rect.top) / nearest.rect.height, clientX: event.clientX, clientY: event.clientY };
      }
      current.onZoomChange(nextZoom);
    };
    host.addEventListener("wheel", onWheel, { passive: false });
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(() => measure());
    observer?.observe(host);
    return () => { host.removeEventListener("wheel", onWheel); observer?.disconnect(); };
  }, [measure]);

  return <main ref={hostRef} className="viewport document-viewport" aria-label="Document" onScroll={(event) => { props.onScrollPositionChange?.({top:event.currentTarget.scrollTop,left:event.currentTarget.scrollLeft}); measure(true); }}
    onPointerDownCapture={(event) => {
      if (props.interactionDisabled) return;
      const page = (event.target as Element).closest<HTMLElement>("[data-page-id]");
      if (page) setActivePage(page.dataset.pageId ?? null);
    }}
    onPointerUpCapture={() => setActivePage(null)} onPointerCancelCapture={() => setActivePage(null)} onLostPointerCapture={() => setActivePage(null)}>
    <div className="document-pages">
      {renderedPages.map((page) => {
        const dimensions = displayDimensions(page.width, page.height, page.rotation);
        const pageNumber = props.pages.indexOf(page) + 1;
        const mounted = nearby.has(page.id) || activePage === page.id || props.viewMode === "single";
        return <div key={page.id} className="document-page" data-page-id={page.id} aria-label={`Page ${pageNumber} container`}
          ref={(element) => { if (element) slots.current.set(page.id, element); else slots.current.delete(page.id); }}
          style={{ width: dimensions.width * props.zoom / 100, height: dimensions.height * props.zoom / 100 }}>
          {mounted ? <PageView adapter={props.adapter} page={page} pageNumber={pageNumber} zoom={props.zoom} tool={props.tool}
            selectedOverlayId={page.id === props.selectedPageId ? props.selectedOverlayId : null}
            pendingSignature={props.pendingSignature} drawColor={props.penColor} drawWidth={props.penWidth} interactionDisabled={props.interactionDisabled}
            onActivate={() => props.onSelectPage(page.id)}
            onSelectOverlay={(id) => props.onSelectOverlay(page.id, id)}
            onAddText={(point) => props.onAddText(page.id, point)}
            onPlaceSignature={(point) => props.onPlaceSignature(page.id, point)}
            onMoveOverlay={(id, x, y, phase) => props.onMoveOverlay(page.id, id, x, y, phase)}
            onDraw={(path) => props.onDraw(page.id, path)} />
            : <div className="page-placeholder" aria-label={`Page ${pageNumber} preview`} />}
        </div>;
      })}
    </div>
  </main>;
}