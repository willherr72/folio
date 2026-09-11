import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { FolioAdapter } from "../editor/adapter";
import { displayDimensions, toDisplayPoint } from "../editor/geometry";
import type { AnnotationRect, EditableTextRun, InkPoint, PagePlan } from "../editor/types";
import type { SearchMatch, SearchRect } from "../editor/search";
import { PageView } from "./PageView";

export interface DocumentViewportProps {
  searchMatches?: SearchMatch[];
  activeSearchMatchId?: string | null;
  initialScrollPosition?: { top: number; left: number };
  onScrollPositionChange?(position: { top: number; left: number }): void;
  adapter: FolioAdapter;
  pages: PagePlan[];
  selectedPageId: string | null;
  selectedOverlayId: string | null;
  zoom: number;
  onZoomChange(zoom: number): void;
  viewMode: "continuous" | "single";
  tool: "select" | "text" | "signature" | "draw" | "highlight" | "comment" | "edit";
  onEditText?(pageId: string, run: EditableTextRun): void;
  penColor: string;
  penWidth: number;
  pendingSignature: InkPoint[][] | null;
  interactionDisabled: boolean;
  navigationRequest: { pageId: string; revision: number; rect?: SearchRect } | null;
  onSelectPage(id: string): void;
  onSelectOverlay(pageId: string, id: string | null): void;
  onAddText(pageId: string, point: InkPoint): void;
  onPlaceSignature(pageId: string, point: InkPoint): void;
  onMoveOverlay(pageId: string, id: string, x: number, y: number, phase: "start" | "move" | "end"): void;
  onDraw(pageId: string, path: InkPoint[]): void;
  onHighlight?(pageId: string, rects: AnnotationRect[]): void;
  onAddComment?(pageId: string, point: InkPoint): void;
}

interface ZoomAnchor { pageId: string; x: number; y: number; clientX: number; clientY: number }

export function DocumentViewport(props: DocumentViewportProps) {
  const searchByPage = useMemo(() => {
    const grouped = new Map<string, SearchMatch[]>();
    for (const match of props.searchMatches ?? []) {
      const matches = grouped.get(match.pageId) ?? [];
      matches.push(match);
      grouped.set(match.pageId, matches);
    }
    return grouped;
  }, [props.searchMatches]);
  const hostRef = useRef<HTMLElement>(null);
  const slots = useRef(new Map<string, HTMLDivElement>());
  const latest = useRef(props);
  latest.current = props;
  const [nearby, setNearby] = useState<Set<string>>(() => new Set([props.selectedPageId ?? props.pages[0]?.id]));
  const [activePage, setActivePage] = useState<string | null>(null);
  const textGesture = useRef(false);
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
    setNearby((previous) => {
      // Native ranges depend on their text nodes, including pages traversed
      // while scrolling. Release the retained pages after pointerup is consumed.
      if (textGesture.current) for (const id of previous) next.add(id);
      return previous.size === next.size && [...next].every((id) => previous.has(id)) ? previous : next;
    });
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

  useEffect(() => {
    let releaseTimer: ReturnType<typeof setTimeout> | undefined;
    const release = () => {
      clearTimeout(releaseTimer);
      // Native document listeners finish gathering selected glyphs first.
      releaseTimer = setTimeout(() => { textGesture.current = false; setActivePage(null); measure(); }, 0);
    };
    document.addEventListener("pointerup", release);
    document.addEventListener("pointercancel", release);
    return () => { clearTimeout(releaseTimer); document.removeEventListener("pointerup", release); document.removeEventListener("pointercancel", release); };
  }, [measure]);
  useEffect(() => { textGesture.current = false; measure(); }, [props.tool, props.viewMode, props.interactionDisabled, measure]);

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
    const page = props.pages.find((item) => item.id === targetId);
    const matchRect = !modeChanged ? request?.rect : undefined;
    if (page && matchRect) {
      const start = toDisplayPoint({ x: matchRect.x, y: matchRect.y }, page.width, page.height, page.rotation);
      const end = toDisplayPoint({ x: matchRect.x + matchRect.width, y: matchRect.y + matchRect.height }, page.width, page.height, page.rotation);
      const scale = props.zoom / 100;
      const width = Math.abs(end.x - start.x) * scale, height = Math.abs(end.y - start.y) * scale;
      host.scrollLeft = Math.max(0, host.scrollLeft + rect.left - viewport.left + Math.min(start.x, end.x) * scale - Math.max(24, (viewport.width - width) / 2));
      host.scrollTop = Math.max(0, host.scrollTop + rect.top - viewport.top + Math.min(start.y, end.y) * scale - Math.max(24, (viewport.height - height) / 2));
    } else host.scrollTop += rect.top - viewport.top - 38;
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
      textGesture.current = !!(event.target as Element).closest('[data-selectable="true"]');
      const page = (event.target as Element).closest<HTMLElement>("[data-page-id]");
      if (page) setActivePage(page.dataset.pageId ?? null);
    }}
    onPointerUpCapture={() => { if (!textGesture.current) setActivePage(null); }} onPointerCancelCapture={() => { if (!textGesture.current) setActivePage(null); }} onLostPointerCapture={() => { if (!textGesture.current) setActivePage(null); }}>
    <div className="document-pages">
      {renderedPages.map((page) => {
        const dimensions = displayDimensions(page.width, page.height, page.rotation);
        const pageNumber = props.pages.indexOf(page) + 1;
        const mounted = nearby.has(page.id) || activePage === page.id || props.viewMode === "single";
        return <div key={page.id} className="document-page" data-page-id={page.id} aria-label={`Page ${pageNumber} container`}
          ref={(element) => { if (element) slots.current.set(page.id, element); else slots.current.delete(page.id); }}
          style={{ width: dimensions.width * props.zoom / 100, height: dimensions.height * props.zoom / 100 }}>
          {mounted ? <PageView adapter={props.adapter} page={page} pageNumber={pageNumber} zoom={props.zoom} tool={props.tool}
            searchMatches={searchByPage.get(page.id)} activeSearchMatchId={props.activeSearchMatchId}
            selectedOverlayId={page.id === props.selectedPageId ? props.selectedOverlayId : null}
            pendingSignature={props.pendingSignature} drawColor={props.penColor} drawWidth={props.penWidth} interactionDisabled={props.interactionDisabled}
            onActivate={() => props.onSelectPage(page.id)}
            onSelectOverlay={(id) => props.onSelectOverlay(page.id, id)}
            onAddText={(point) => props.onAddText(page.id, point)}
            onEditText={(run) => props.onEditText?.(page.id, run)}
            onPlaceSignature={(point) => props.onPlaceSignature(page.id, point)}
            onMoveOverlay={(id, x, y, phase) => props.onMoveOverlay(page.id, id, x, y, phase)}
            onHighlight={(rects) => props.onHighlight?.(page.id, rects)}
            onAddComment={(point) => props.onAddComment?.(page.id, point)}
            onDraw={(path) => props.onDraw(page.id, path)} />
            : <div className="page-placeholder" aria-label={`Page ${pageNumber} preview`} />}
        </div>;
      })}
    </div>
  </main>;
}
