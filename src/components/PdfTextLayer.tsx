import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { FolioAdapter } from "../editor/adapter";
import type { AnnotationRect, PagePlan, PageText } from "../editor/types";
import "./page-interactions.css";
import "./annotations.css";
import { registerHighlightLayer } from "../editor/annotation-selection";
import { selectableTextSpans, selectedScalarText } from "../editor/selectable-text";
import {rectVisibleOnPage} from "../editor/geometry";

import { readPageReadingText } from "../editor/page-reading-text";
import { clearSearchTextCache, readSearchPageText } from "../editor/search";
export const clearPageTextCache = clearSearchTextCache;

// Extraction objects live in the bounded source-text cache. Weak keys let
// their measured geometry disappear when that cache/source is released.
const measuredScales = new WeakMap<PageText, Float64Array>();

function usePageText(adapter: FolioAdapter, page: PagePlan, needsGeometry: boolean) {
  const canCopy = page.protection?.permissions.copyContent !== false;
  const key = JSON.stringify([page.sourceId, page.pageIndex]);
  const compact = !needsGeometry && !!adapter.getPageReadingText;
  const richProvider = adapter.getPageText, readingProvider = adapter.getPageReadingText;
  type State = {adapter: FolioAdapter; key: string; richProvider: typeof richProvider;
    readingProvider: typeof readingProvider; text?: PageText; reading?: string};
  const [state, setState] = useState<State>({adapter, key, richProvider, readingProvider});
  const owned = state.adapter === adapter && state.key === key &&
    state.richProvider === richProvider && state.readingProvider === readingProvider;
  useEffect(() => {
    let active = true;
    setState(previous => ({adapter, key, richProvider, readingProvider,
      reading: canCopy && !page.crop && previous.adapter === adapter && previous.key === key &&
        previous.richProvider === richProvider && previous.readingProvider === readingProvider ? previous.reading : undefined}));
    if (!canCopy || (!compact && !richProvider)) return;
    const request = compact
      ? readPageReadingText(adapter, page.sourceId, page.pageIndex).then(reading => ({reading}))
      : readSearchPageText(adapter, page.sourceId, page.pageIndex).then(text =>
          ({text, reading: text.characters.map(character => character.text).join("")}));
    request.then(result => {
      if (active) setState({adapter, key, richProvider, readingProvider, ...result});
    }).catch(() => {
      // Unavailable text must not interrupt viewing or fall back around permissions.
    });
    return () => { active = false; };
  }, [adapter, key, page.sourceId, page.pageIndex, canCopy, compact, richProvider, readingProvider]);
  return {
    text: canCopy && owned && !compact ? state.text : undefined,
    reading: canCopy && owned && !page.crop ? state.reading : undefined,
  };
}

export function PdfTextLayer({ adapter, page, pageNumber, selectable: requestedSelectable, highlighting = false, onHighlight }: {
  adapter: FolioAdapter; page: PagePlan; pageNumber: number; selectable: boolean; highlighting?: boolean; onHighlight?(rects: AnnotationRect[]): void;
}) {
  const {text, reading} = usePageText(adapter, page, requestedSelectable || !!page.crop);
  const selectable = requestedSelectable && !!text;
  const intrinsicRotation = text?.intrinsicRotation ?? 0;
  const sideways = intrinsicRotation === 90 || intrinsicRotation === 270;
  // Uncropped logical reading needs no hit targets or duplicated glyph boxes.
  const characters = useMemo(() => text && (selectable || page.crop) ? selectableTextSpans(text) : [], [text, selectable, page.crop]);
  const scales = text&&!page.crop ? measuredScales.get(text) : undefined;
  const logicalRuns = useMemo(() => {
    if (selectable) return [];
    if (!page.crop) return reading ? [{ start: 0, text: reading, hidden: false }] : [];
    if (!text) return [];
    const runs: Array<{ start: number; text: string; hidden: boolean }> = [];
    for (const character of characters) {
      const hidden = !!page.crop && !rectVisibleOnPage({ ...character, width: Math.max(.001, character.width), height: Math.max(.001, character.height) }, page);
      const previous = runs[runs.length - 1];
      if (previous && previous.hidden === hidden) previous.text += character.text;
      else runs.push({ start: character.start, text: character.text, hidden });
    }
    return runs;
  }, [characters, text, reading, selectable, page.crop]);
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
    if (!selectable || !text || scales) return;
    const spans = layerRef.current?.querySelectorAll<HTMLElement>("[data-pdf-character]");
    if (!spans?.length) return;
    // A sibling duplicate can populate the cache after this layer rendered.
    // Apply that result too, without measuring the same source page twice.
    let measured = page.crop?undefined:measuredScales.get(text);
    if (!measured) {
      // Reads before writes avoid per-glyph forced layouts on the first visit.
      const widths = Array.from(spans, span => span.offsetWidth);
      measured = new Float64Array(widths.length);
      characters.forEach((character, index) => {
        const width = sideways ? character.height : character.width;
        measured![index] = width > 0 && widths[index] > 0 ? width / widths[index] : 1;
      });
      if(!page.crop)measuredScales.set(text, measured);
    }
    spans.forEach((span, index) => { span.style.transform = `rotate(${intrinsicRotation}deg) scaleX(${measured![index]})`; });
  }, [selectable, text, intrinsicRotation, sideways, scales, characters,page.crop]);

  useLayoutEffect(() => {
    if (!selectable) return;
    const copy = (event: ClipboardEvent) => {
      const selection = window.getSelection();
      const layer = layerRef.current;
      if (event.defaultPrevented || !event.clipboardData || !selection?.rangeCount || selection.isCollapsed || !layer?.contains(selection.anchorNode)) return;
      const target = event.target instanceof Element ? event.target : null;
      if (target?.closest("input, textarea, [contenteditable=true]")) return;
      // Browser layout inserts separators between positioned spans. Extract the
      // selected character nodes instead, retaining PDFium's exact whitespace.
      const scope = layer.closest(".document-viewport");
      const layers = scope ? scope.querySelectorAll('.pdf-text-layer[data-selectable="true"]') : [layer];
      const fragments: string[] = [];
      for (const selectedLayer of layers) {
        const pieces: string[] = [];
        for (let index = 0; index < selection.rangeCount; index++) {
          const range = selection.getRangeAt(index);
          selectedLayer.querySelectorAll('[data-pdf-character]').forEach((span) => {
            if((span as HTMLElement).hidden)return;
            const node = span.firstChild;
            if (!node || !range.intersectsNode(node)) return;
            const value = node.textContent ?? "";
            const start = range.startContainer === node ? range.startOffset : 0;
            const end = range.endContainer === node ? range.endOffset : value.length;
            pieces.push(selectedScalarText(value, start, end));
          });
        }
        const fragment = pieces.join("");
        if (fragment) fragments.push(fragment);
      }
      let copied = "";
      for (const fragment of fragments) {
        if (copied && !/[\r\n]$/.test(copied) && !/^[\r\n]/.test(fragment)) copied += "\n";
        copied += fragment;
      }
      event.clipboardData.setData("text/plain", copied);
      event.preventDefault();
    };
    document.addEventListener("copy", copy);
    return () => document.removeEventListener("copy", copy);
  }, [selectable]);

  return <foreignObject className="pdf-text-host" width={page.width} height={page.height} pointerEvents={selectable ? "auto" : "none"}>
    <div ref={layerRef} className="pdf-text-layer" role="document" aria-label={`Page ${pageNumber} text`} data-selectable={selectable} data-highlighting={highlighting && selectable}>
      {selectable ? characters.map((character, index) => {
        // Bounds already include source rotation. Restore the glyph's local axes
        // within that rectangle so browser carets face the same way as the PDF.
        const left = character.x + (intrinsicRotation === 90 || intrinsicRotation === 180 ? character.width : 0);
        const top = character.y + (intrinsicRotation === 180 || intrinsicRotation === 270 ? character.height : 0);
        const height = Math.max(1, sideways ? character.width : character.height);
        return <span key={character.start} hidden={!!page.crop&&!rectVisibleOnPage({...character,width:Math.max(.001,character.width),height:Math.max(.001,character.height)},page)} data-pdf-character={character.start} data-pdf-character-end={character.end} style={{ left, top, fontSize: height, height, transform: scales ? `rotate(${intrinsicRotation}deg) scaleX(${scales[index]})` : undefined }}>{character.text}</span>;
      }) : logicalRuns.map(run => <span key={run.start} className="pdf-logical-text" hidden={run.hidden}>{run.text}</span>)}
      {highlighting && ((!adapter.getPageText) || (text && !text.characters.some(character => character.text.trim()))) && <div className="annotation-empty-cue">No embedded text to highlight on this page. Use a comment instead.</div>}
    </div>
  </foreignObject>;
}
