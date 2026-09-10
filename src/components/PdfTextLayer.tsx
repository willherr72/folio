import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { FolioAdapter } from "../editor/adapter";
import type { PagePlan, PageText } from "../editor/types";
import "./page-interactions.css";

const textCache = new Map<string, { sourceId: string; request: Promise<PageText> }>();
const MAX_TEXT_CACHE = 60;

export function clearPageTextCache(sourceIds: string[]) {
  const targets = new Set(sourceIds);
  for (const [key, entry] of textCache) if (targets.has(entry.sourceId)) textCache.delete(key);
}

function usePageText(adapter: FolioAdapter, page: PagePlan) {
  const key = `${adapter.kind}:${page.sourceId}:${page.pageIndex}`;
  const [state, setState] = useState<{ key: string; text?: PageText }>({ key });
  useEffect(() => {
    let active = true;
    setState({ key });
    if (!adapter.getPageText) return;
    let entry = textCache.get(key);
    if (!entry) {
      entry = { sourceId: page.sourceId, request: adapter.getPageText(page.sourceId, page.pageIndex) };
      textCache.set(key, entry);
    } else {
      textCache.delete(key);
      textCache.set(key, entry);
    }
    while (textCache.size > MAX_TEXT_CACHE) textCache.delete(textCache.keys().next().value!);
    entry.request.then((text) => { if (active) setState({ key, text }); }).catch(() => {
      if (textCache.get(key) === entry) textCache.delete(key);
      // A scanned page or an unavailable text API must not interrupt viewing.
    });
    return () => { active = false; };
  }, [adapter, key, page.sourceId, page.pageIndex]);
  return state.key === key ? state.text : undefined;
}

export function PdfTextLayer({ adapter, page, pageNumber, selectable }: {
  adapter: FolioAdapter; page: PagePlan; pageNumber: number; selectable: boolean;
}) {
  const text = usePageText(adapter, page);
  const layerRef = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const spans = layerRef.current?.querySelectorAll<HTMLElement>("[data-pdf-character]");
    spans?.forEach((span, index) => {
      const width = text?.characters[index].width ?? 0;
      // offsetWidth is in unrotated CSS pixels, independent of page zoom.
      const naturalWidth = span.offsetWidth;
      span.style.transform = width > 0 && naturalWidth > 0 ? `scaleX(${width / naturalWidth})` : "";
    });
  }, [text]);

  useEffect(() => {
    if (!selectable) return;
    const copy = (event: ClipboardEvent) => {
      const selection = window.getSelection();
      const layer = layerRef.current;
      if (event.defaultPrevented || !event.clipboardData || !selection?.rangeCount || selection.isCollapsed || !layer?.contains(selection.anchorNode)) return;
      const target = event.target instanceof Element ? event.target : null;
      if (target?.closest("input, textarea, [contenteditable=true]")) return;
      // Browser layout inserts separators between positioned spans. Extract the
      // selected character nodes instead, retaining PDFium's exact whitespace.
      const pieces: string[] = [];
      for (let index = 0; index < selection.rangeCount; index++) {
        const range = selection.getRangeAt(index);
        document.querySelectorAll('[data-selectable="true"] [data-pdf-character]').forEach((span) => {
          const node = span.firstChild;
          if (!node || !range.intersectsNode(node)) return;
          const value = node.textContent ?? "";
          const start = range.startContainer === node ? range.startOffset : 0;
          const end = range.endContainer === node ? range.endOffset : value.length;
          pieces.push(value.slice(start, end));
        });
      }
      event.clipboardData.setData("text/plain", pieces.join(""));
      event.preventDefault();
    };
    document.addEventListener("copy", copy);
    return () => document.removeEventListener("copy", copy);
  }, [selectable]);

  return <foreignObject className="pdf-text-host" width={page.width} height={page.height} pointerEvents={selectable ? "auto" : "none"}>
    <div ref={layerRef} className="pdf-text-layer" role="document" aria-label={`Page ${pageNumber} text`} data-selectable={selectable}>
      {text?.characters.map((character, index) => <span key={index} data-pdf-character="" style={{
        left: character.x, top: character.y, fontSize: Math.max(1, character.height),
        height: Math.max(1, character.height),
      }}>{character.text}</span>)}
    </div>
  </foreignObject>;
}