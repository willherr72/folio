import type { AnnotationRect, PdfTextCharacter, Rotation } from "./types";

/** Merge selected glyphs along their source reading axis, never in screen pixels. */
export function mergeHighlightCharacters(characters: PdfTextCharacter[], rotation: Rotation = 0): AnnotationRect[] {
  const vertical = rotation === 90 || rotation === 270;
  const rects: AnnotationRect[] = [];
  let current: AnnotationRect | undefined;
  for (const character of characters) {
    if (/[\r\n]/.test(character.text)) { current = undefined; continue; }
    if (!character.text.trim() || character.width <= 0 || character.height <= 0 || ![character.x, character.y, character.width, character.height].every(Number.isFinite)) continue;
    const rect = { x: character.x, y: character.y, width: character.width, height: character.height };
    if (current) {
      const crossStart = vertical ? rect.x : rect.y;
      const crossEnd = crossStart + (vertical ? rect.width : rect.height);
      const previousStart = vertical ? current.x : current.y;
      const previousEnd = previousStart + (vertical ? current.width : current.height);
      const overlap = Math.min(crossEnd, previousEnd) - Math.max(crossStart, previousStart);
      const alongGap = vertical
        ? Math.max(rect.y - current.y - current.height, current.y - rect.y - rect.height)
        : Math.max(rect.x - current.x - current.width, current.x - rect.x - rect.width);
      if (overlap >= Math.min(crossEnd - crossStart, previousEnd - previousStart) * .5 && alongGap <= Math.max(crossEnd - crossStart, previousEnd - previousStart) * 2) {
        const right = Math.max(current.x + current.width, rect.x + rect.width);
        const bottom = Math.max(current.y + current.height, rect.y + rect.height);
        current.x = Math.min(current.x, rect.x); current.y = Math.min(current.y, rect.y);
        current.width = right - current.x; current.height = bottom - current.y;
        continue;
      }
    }
    current = rect; rects.push(rect);
  }
  return rects;
}

export function selectedHighlightRects(layer: HTMLElement, characters: PdfTextCharacter[], selection: Selection, rotation: Rotation = 0): AnnotationRect[] {
  if (selection.isCollapsed) return [];
  const selected = new Set<number>();
  for (let index = 0; index < selection.rangeCount; index++) {
    const range = selection.getRangeAt(index);
    layer.querySelectorAll<HTMLElement>("[data-pdf-character]").forEach((span) => {
      const node = span.firstChild;
      if (!node || !range.intersectsNode(node)) return;
      const start = range.startContainer === node ? range.startOffset : 0;
      const end = range.endContainer === node ? range.endOffset : (node.textContent?.length ?? 0);
      if (end > start) selected.add(Number(span.dataset.pdfCharacter));
    });
  }
  return mergeHighlightCharacters(characters.filter((_, index) => selected.has(index)), rotation);
}
interface HighlightLayer {
  characters: PdfTextCharacter[];
  rotation: Rotation;
  onHighlight(rects: AnnotationRect[]): void;
}
const highlightLayers = new Map<HTMLElement, HighlightLayer>();
let gesture: { origin: HTMLElement; pointer: number; viewport: Element | null } | null = null;
function startHighlightGesture(event: PointerEvent) {
  const origin = event.target instanceof Element ? event.target.closest<HTMLElement>('[data-highlighting="true"]') : null;
  gesture = event.button === 0 && origin && highlightLayers.has(origin) ? { origin, pointer: event.pointerId, viewport: origin.closest(".document-viewport") } : null;
}
function cancelHighlightGesture() { gesture = null; }
function finishHighlightGesture(event: PointerEvent) {
  if (!gesture || gesture.pointer !== event.pointerId) return;
  const current = gesture; gesture = null;
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed) return;
  // A single native listener gathers all pages before any callback can render,
  // unmount a layer, or clear the range. Newly mounted pages join the registry.
  const changes: Array<{ layer: HighlightLayer; rects: AnnotationRect[] }> = [];
  const layers = [...highlightLayers].sort(([a], [b]) => a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1);
  for (const [element, layer] of layers) {
    if (element.closest(".document-viewport") !== current.viewport) continue;
    const rects = selectedHighlightRects(element, layer.characters, selection, layer.rotation);
    if (rects.length) changes.push({ layer, rects });
  }
  if (!changes.length) return;
  selection.removeAllRanges();
  for (const { layer, rects } of changes) layer.onHighlight(rects);
}
export function registerHighlightLayer(element: HTMLElement, layer: HighlightLayer) {
  if (!highlightLayers.size) {
    document.addEventListener("pointerdown", startHighlightGesture, true);
    document.addEventListener("pointerup", finishHighlightGesture);
    document.addEventListener("pointercancel", cancelHighlightGesture);
  }
  highlightLayers.set(element, layer);
  return () => {
    highlightLayers.delete(element);
    if (gesture?.origin === element) gesture = null;
    if (!highlightLayers.size) {
      document.removeEventListener("pointerdown", startHighlightGesture, true);
      document.removeEventListener("pointerup", finishHighlightGesture);
      document.removeEventListener("pointercancel", cancelHighlightGesture);
    }
  };
}