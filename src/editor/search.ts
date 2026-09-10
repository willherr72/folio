import { useEffect, useState } from "react";
import type { FolioAdapter } from "./adapter";
import { textOverlayCharacters } from "./text-overlay-geometry";
import type { PagePlan, PageText, PdfTextCharacter } from "./types";

/** Coordinates are page units, after source rotation and before editor rotation. */
export interface SearchRect { x: number; y: number; width: number; height: number }
export interface SearchMatch { id: string; pageId: string; rects: SearchRect[] }

interface TextEntry { adapter: FolioAdapter; sourceId: string; request: Promise<PageText>; characters: number }
const textCache = new Map<string, TextEntry>();
const MAX_TEXT_PAGES = 60;
const MAX_TEXT_CHARACTERS = 250_000;

function pruneTextCache() {
  let characters = Array.from(textCache.values()).reduce((sum, entry) => sum + entry.characters, 0);
  while (textCache.size > MAX_TEXT_PAGES || characters > MAX_TEXT_CHARACTERS) {
    const key = textCache.keys().next().value;
    if (key === undefined) break;
    characters -= textCache.get(key)!.characters;
    textCache.delete(key);
  }
}

export function clearSearchTextCache(sourceIds: string[]) {
  const sources = new Set(sourceIds);
  for (const [key, entry] of textCache) if (sources.has(entry.sourceId)) textCache.delete(key);
}

/** Share pending and completed extraction with the selectable PDF text layer. */
export function readSearchPageText(adapter: FolioAdapter, sourceId: string, pageIndex: number): Promise<PageText> {
  if (!adapter.getPageText) return Promise.resolve({ characters: [] });
  const key = JSON.stringify([sourceId, pageIndex]);
  let entry = textCache.get(key);
  if (entry?.adapter !== adapter) entry = undefined;
  if (!entry) {
    const next: TextEntry = { adapter, sourceId, characters: 0, request: Promise.resolve({ characters: [] }) };
    next.request = Promise.resolve().then(() => adapter.getPageText!(sourceId, pageIndex)).then((text) => {
      next.characters = text.characters.length;
      pruneTextCache();
      return text;
    }).catch((error: unknown) => {
      if (textCache.get(key) === next) textCache.delete(key);
      throw error;
    });
    entry = next;
  }
  textCache.delete(key);
  textCache.set(key, entry);
  pruneTextCache();
  return entry.request;
}

export function normalizeSearchQuery(query: string): string { return query.replace(/\s+/gu, " ").trim().toLowerCase(); }

function normalizeCharacters(characters: PdfTextCharacter[]) {
  const letters: string[] = [];
  const glyphs: number[] = [];
  for (let index = 0; index < characters.length; index++) {
    for (const character of characters[index].text) {
      if (/\s/u.test(character)) {
        if (letters.length && letters[letters.length - 1] !== " ") { letters.push(" "); glyphs.push(index); }
      } else {
        // String search offsets use UTF-16 units, including case expansions.
        const lower = character.toLowerCase();
        for (let unit = 0; unit < lower.length; unit++) { letters.push(lower[unit]); glyphs.push(index); }
      }
    }
  }
  if (letters[letters.length - 1] === " ") { letters.pop(); glyphs.pop(); }
  return { value: letters.join(""), glyphs };
}

function mergeRects(characters: PdfTextCharacter[], vertical: boolean): SearchRect[] {
  const rects: SearchRect[] = [];
  for (const character of characters) {
    if (!character.text.trim() || character.width <= 0 || character.height <= 0) continue;
    const { x, y, width, height } = character;
    if (![x, y, width, height].every(Number.isFinite)) continue;
    const last = rects[rects.length - 1];
    const overlap = last ? vertical
      ? Math.min(last.x + last.width, x + width) - Math.max(last.x, x)
      : Math.min(last.y + last.height, y + height) - Math.max(last.y, y) : 0;
    const gap = last ? vertical
      ? Math.max(y - last.y - last.height, last.y - y - height, 0)
      : Math.max(x - last.x - last.width, last.x - x - width, 0) : Infinity;
    const thickness = last ? vertical ? Math.min(last.width, width) : Math.min(last.height, height) : 0;
    if (last && overlap >= thickness * 0.5 && gap <= thickness) {
      const right = Math.max(last.x + last.width, x + width), bottom = Math.max(last.y + last.height, y + height);
      last.x = Math.min(last.x, x); last.y = Math.min(last.y, y);
      last.width = right - last.x; last.height = bottom - last.y;
    } else rects.push({ x, y, width, height });
  }
  return rects;
}

function findTextMatches(pageId: string, scope: string, text: PageText, query: string): SearchMatch[] {
  const normalized = normalizeCharacters(text.characters);
  const matches: SearchMatch[] = [];
  const vertical = text.intrinsicRotation === 90 || text.intrinsicRotation === 270;
  let offset = normalized.value.indexOf(query);
  while (offset !== -1) {
    const start = normalized.glyphs[offset], end = normalized.glyphs[offset + query.length - 1];
    const rects = mergeRects(text.characters.slice(start, end + 1), vertical);
    matches.push({ id: `${pageId}:${scope}:${offset}`, pageId, rects });
    offset = normalized.value.indexOf(query, offset + query.length);
  }
  return matches;
}

export function findPageMatches(page: PagePlan, text: PageText, query: string): SearchMatch[] {
  const needle = normalizeSearchQuery(query);
  if (!needle) return [];
  const matches = findTextMatches(page.id, "source", text, needle);
  for (const overlay of page.overlays) {
    if (overlay.type === "text") matches.push(...findTextMatches(page.id, `overlay:${overlay.id}`, textOverlayCharacters(overlay), needle));
  }
  return matches;
}

interface SearchResult { matches: SearchMatch[]; searching: boolean; error: string | null; hasText: boolean }
interface SearchState extends SearchResult { adapter: FolioAdapter; pages: PagePlan[]; query: string }

export function useDocumentSearch(adapter: FolioAdapter, pages: PagePlan[], query: string, enabled: boolean): SearchResult {
  const needle = normalizeSearchQuery(query);
  const [state, setState] = useState<SearchState | null>(null);
  useEffect(() => {
    if (!enabled || !needle) return;
    let cancelled = false;
    const timer = setTimeout(() => {
      void (async () => {
        const matches: SearchMatch[] = [];
        let hasText = false;
        const errors: string[] = [];
        for (let index = 0; index < pages.length; index++) {
          if (cancelled) return;
          const page = pages[index];
          let text: PageText = { characters: [] };
          try { text = await readSearchPageText(adapter, page.sourceId, page.pageIndex); }
          catch (error) { errors.push(`Page ${index + 1}: ${error instanceof Error ? error.message : String(error)}`); }
          if (cancelled) return;
          hasText ||= text.characters.some((character) => !!character.text.trim()) || page.overlays.some((overlay) => overlay.type === "text" && !!overlay.text.trim());
          matches.push(...findPageMatches(page, text, needle));
          setState({ adapter, pages, query: needle, matches: [...matches], hasText, error: errors.length ? `Some pages could not be searched. ${errors[0]}` : null, searching: index < pages.length - 1 });
          // Let rendering and input run even when every extraction is cached.
          if (index < pages.length - 1) await new Promise<void>((resolve) => setTimeout(resolve, 0));
        }
        if (!pages.length && !cancelled) setState({ adapter, pages, query: needle, matches: [], hasText: false, error: null, searching: false });
      })();
    }, 120);
    return () => { cancelled = true; clearTimeout(timer); };
  }, [adapter, pages, needle, enabled]);

  if (!enabled || !needle) return { matches: [], searching: false, error: null, hasText: false };
  if (!state || state.adapter !== adapter || state.pages !== pages || state.query !== needle) return { matches: [], searching: true, error: null, hasText: false };
  return state;
}
