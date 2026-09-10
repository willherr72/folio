import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { FolioAdapter } from "../editor/adapter";
import type { PagePlan, PageText } from "../editor/types";
import "./page-interactions.css";

import { clearSearchTextCache, readSearchPageText } from "../editor/search";
export const clearPageTextCache = clearSearchTextCache;

function usePageText(adapter: FolioAdapter, page: PagePlan) {
  const key = `${adapter.kind}:${page.sourceId}:${page.pageIndex}`;
  const [state, setState] = useState<{ key: string; text?: PageText }>({ key });
  useEffect(() => {
    let active = true;
    setState({ key });
    if (!adapter.getPageText) return;
    readSearchPageText(adapter, page.sourceId, page.pageIndex).then((text) => {
      if (active) setState({ key, text });
    }).catch(() => {
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
  const intrinsicRotation = text?.intrinsicRotation ?? 0;
  const sideways = intrinsicRotation === 90 || intrinsicRotation === 270;
  const layerRef = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const spans = layerRef.current?.querySelectorAll<HTMLElement>("[data-pdf-character]");
    // Finish all layout reads before writing transforms: interleaving these
    // forces a full text-layer layout for every character on a tab remount.
    const widths = Array.from(spans ?? [], span => span.offsetWidth);
    spans?.forEach((span, index) => {
      const character = text?.characters[index];
      const width = (sideways ? character?.height : character?.width) ?? 0;
      // offsetWidth is in unrotated CSS pixels, independent of page zoom.
      const naturalWidth = widths[index];
      span.style.transform = `rotate(${intrinsicRotation}deg) scaleX(${width > 0 && naturalWidth > 0 ? width / naturalWidth : 1})`;
    });
  }, [text, intrinsicRotation, sideways]);

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
      {text?.characters.map((character, index) => {
        // Bounds already include source rotation. Restore the glyph's local axes
        // within that rectangle so browser carets face the same way as the PDF.
        const left = character.x + (intrinsicRotation === 90 || intrinsicRotation === 180 ? character.width : 0);
        const top = character.y + (intrinsicRotation === 180 || intrinsicRotation === 270 ? character.height : 0);
        const height = Math.max(1, sideways ? character.width : character.height);
        return <span key={index} data-pdf-character="" style={{ left, top, fontSize: height, height }}>{character.text}</span>;
      })}
    </div>
  </foreignObject>;
}