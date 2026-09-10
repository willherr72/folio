import type { EditorDocument, History, Overlay, PagePlan, Rotation } from "./types";

export type { EditorDocument, History, Overlay, PagePlan, Rotation } from "./types";

export function createHistory<T>(initial: T): History<T> {
  return { past: [], present: initial, future: [] };
}

export function commit<T>(history: History<T>, update: (current: T) => T): History<T> {
  const next = update(history.present);
  if (next === history.present) return history;
  return { past: [...history.past, history.present], present: next, future: [] };
}

export function undo<T>(history: History<T>): History<T> {
  const previous = history.past.at(-1);
  if (previous === undefined) return history;
  return {
    past: history.past.slice(0, -1),
    present: previous,
    future: [history.present, ...history.future],
  };
}

export function redo<T>(history: History<T>): History<T> {
  const next = history.future[0];
  if (next === undefined) return history;
  return {
    past: [...history.past, history.present],
    present: next,
    future: history.future.slice(1),
  };
}

function cloneOverlay(overlay: Overlay): Overlay {
  return overlay.type === "text"
    ? { ...overlay }
    : { ...overlay, paths: overlay.paths.map((path) => path.map((point) => ({ ...point }))) };
}

export function duplicatePage(document: EditorDocument, pageId: string, newId: string): EditorDocument {
  const index = document.pages.findIndex((page) => page.id === pageId);
  if (index < 0) return document;
  const source = document.pages[index];
  const duplicate: PagePlan = { ...source, id: newId, overlays: source.overlays.map(cloneOverlay) };
  const pages = [...document.pages];
  pages.splice(index + 1, 0, duplicate);
  return { ...document, pages, selectedPageId: duplicate.id, selectedOverlayId: null };
}

export function movePage(document: EditorDocument, pageId: string, destination: number): EditorDocument {
  const index = document.pages.findIndex((page) => page.id === pageId);
  if (index < 0) return document;
  const pages = [...document.pages];
  const [page] = pages.splice(index, 1);
  pages.splice(Math.max(0, Math.min(destination, pages.length)), 0, page);
  return { ...document, pages };
}

export function deletePage(document: EditorDocument, pageId: string): EditorDocument {
  const index = document.pages.findIndex((page) => page.id === pageId);
  if (index < 0) return document;
  const pages = document.pages.filter((page) => page.id !== pageId);
  const fallback = pages[Math.min(index, pages.length - 1)]?.id ?? null;
  return {
    ...document,
    pages,
    selectedPageId: document.selectedPageId === pageId ? fallback : document.selectedPageId,
    selectedOverlayId: document.selectedPageId === pageId ? null : document.selectedOverlayId,
  };
}

export function rotatePage(document: EditorDocument, pageId: string, direction: -1 | 1 = 1): EditorDocument {
  const cycle: Rotation[] = [0, 90, 180, 270];
  return {
    ...document,
    pages: document.pages.map((page) => page.id === pageId
      ? { ...page, rotation: cycle[(cycle.indexOf(page.rotation) + direction + 4) % 4] }
      : page),
  };
}

export function updatePage(document: EditorDocument, pageId: string, update: (page: PagePlan) => PagePlan): EditorDocument {
  return { ...document, pages: document.pages.map((page) => page.id === pageId ? update(page) : page) };
}

export function updateOverlay(document: EditorDocument, pageId: string, overlayId: string, update: (overlay: Overlay) => Overlay): EditorDocument {
  return updatePage(document, pageId, (page) => ({
    ...page,
    overlays: page.overlays.map((overlay) => overlay.id === overlayId ? update(overlay) : overlay),
  }));
}

export function removeOverlay(document: EditorDocument, pageId: string, overlayId: string): EditorDocument {
  return {
    ...updatePage(document, pageId, (page) => ({ ...page, overlays: page.overlays.filter((overlay) => overlay.id !== overlayId) })),
    selectedOverlayId: document.selectedOverlayId === overlayId ? null : document.selectedOverlayId,
  };
}

export function planDigest(document: EditorDocument): string {
  return JSON.stringify(document.pages);
}

let nextId = 0;
export function uniqueId(prefix: string): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) return `${prefix}-${crypto.randomUUID()}`;
  nextId += 1;
  return `${prefix}-${Date.now()}-${nextId}`;
}