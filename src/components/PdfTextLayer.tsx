import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { FolioAdapter } from "../editor/adapter";
import type { AnnotationRect, PagePlan, PageText } from "../editor/types";
import "./page-interactions.css";
import "./annotations.css";
import { registerHighlightLayer } from "../editor/annotation-selection";
import { selectableTextSpans, selectedScalarText } from "../editor/selectable-text";

import { clearSearchTextCache, readSearchPageText } from "../editor/search";
export const clearPageTextCache = clearSearchTextCache;

// Extraction objects live in the bounded source-text cache. Weak keys let
// their measured geometry disappear when that cache/source is released.
const measuredScales = new WeakMap<PageText, Float64Array>();

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

export function PdfTextLayer({ adapter, page, pageNumber, selectable, highlighting = false, onHighlight }: {
  adapter: FolioAdapter; page: PagePlan; pageNumber: number; selectable: boolean; highlighting?: boolean; onHighlight?(rects: AnnotationRect[]): void;
}) {
  const text = usePageText(adapter, page);
  const intrinsicRotation = text?.intrinsicRotation ?? 0;
  const sideways = intrinsicRotation === 90 || intrinsicRotation === 270;
  const characters = text ? selectableTextSpans(text) : [];
  const scales = text ? measuredScales.get(text) : undefined;
  const layerRef = useRef<HTMLDivElement>(null);
  const highlightCallback = useRef(onHighlight);
  highlightCallback.current = onHighlight;
  useLayoutEffect(() => {
    if (!highlighting || !selectable || !text) return;
    const layer = layerRef.current;
    if (!layer) return;
    return registerHighlightLayer(layer, { characters: text.characters, rotation: intrinsicRotation, onHighlight: rects => highlightCallback.current?.(rects) });
  }, [highlighting, selectable, text, intrinsicRotation]);
  useLayoutEffect(() => {
    if (!text || scales) return;
    const spans = layerRef.current?.querySelectorAll<HTMLElement>("[data-pdf-character]");
    if (!spans?.length) return;
    // A sibling duplicate can populate the cache after this layer rendered.
    // Apply that result too, without measuring the same source page twice.
    let measured = measuredScales.get(text);
    if (!measured) {
      // Reads before writes avoid per-glyph forced layouts on the first visit.
      const widths = Array.from(spans, span => span.offsetWidth);
      measured = new Float64Array(widths.length);
      characters.forEach((character, index) => {
        const width = sideways ? character.height : character.width;
        measured![index] = width > 0 && widths[index] > 0 ? width / widths[index] : 1;
      });
      measuredScales.set(text, measured);
    }
    spans.forEach((span, index) => { span.style.transform = `rotate(${intrinsicRotation}deg) scaleX(${measured![index]})`; });
  }, [text, intrinsicRotation, sideways, scales, characters]);

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
        const scope = layer.closest(".document-viewport") ?? layer;
        scope.querySelectorAll('[data-selectable="true"] [data-pdf-character]').forEach((span) => {
          const node = span.firstChild;
          if (!node || !range.intersectsNode(node)) return;
          const value = node.textContent ?? "";
          const start = range.startContainer === node ? range.startOffset : 0;
          const end = range.endContainer === node ? range.endOffset : value.length;
          pieces.push(selectedScalarText(value, start, end));
        });
      }
      event.clipboardData.setData("text/plain", pieces.join(""));
      event.preventDefault();
    };
    document.addEventListener("copy", copy);
    return () => document.removeEventListener("copy", copy);
  }, [selectable]);

  return <foreignObject className="pdf-text-host" width={page.width} height={page.height} pointerEvents={selectable ? "auto" : "none"}>
    <div ref={layerRef} className="pdf-text-layer" role="document" aria-label={`Page ${pageNumber} text`} data-selectable={selectable} data-highlighting={highlighting && selectable}>
      {characters.map((character, index) => {
        // Bounds already include source rotation. Restore the glyph's local axes
        // within that rectangle so browser carets face the same way as the PDF.
        const left = character.x + (intrinsicRotation === 90 || intrinsicRotation === 180 ? character.width : 0);
        const top = character.y + (intrinsicRotation === 180 || intrinsicRotation === 270 ? character.height : 0);
        const height = Math.max(1, sideways ? character.width : character.height);
        return <span key={character.start} data-pdf-character={character.start} data-pdf-character-end={character.end} style={{ left, top, fontSize: height, height, transform: scales ? `rotate(${intrinsicRotation}deg) scaleX(${scales[index]})` : undefined }}>{character.text}</span>;
      })}
      {highlighting && ((!adapter.getPageText) || (text && !text.characters.some(character => character.text.trim()))) && <div className="annotation-empty-cue">No embedded text to highlight on this page. Use a comment instead.</div>}
    </div>
  </foreignObject>;
}
