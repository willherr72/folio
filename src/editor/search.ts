import {rectVisibleOnPage} from "./geometry";
import { acquireShapedText } from "./shaped-text";
import { fontRevision, subscribeFonts } from "./custom-fonts";
import { useEffect, useState, useSyncExternalStore } from "react";
import type { FolioAdapter } from "./adapter";
import { textOverlayCharacters, rotateTextRect } from "./text-overlay-geometry";
import type { PagePlan, PageText, PdfTextCharacter,PageSearchResult } from "./types";

/** Coordinates are page units, after source rotation and before editor rotation. */
export interface SearchRect { x: number; y: number; width: number; height: number }
export interface SearchMatch { id: string; pageId: string; rects: SearchRect[] }

/** Navigate to visible ink without changing the complete match used for highlighting. */
export function searchNavigationRect(page: PagePlan, match: SearchMatch): SearchRect | undefined {
  if(!page.crop)return match.rects[0];
  const crop=page.crop;
  for(const rect of match.rects){
    const x=Math.max(rect.x,crop.x),y=Math.max(rect.y,crop.y);
    const width=Math.min(rect.x+rect.width,crop.x+crop.width)-x;
    const height=Math.min(rect.y+rect.height,crop.y+crop.height)-y;
    if(width>0&&height>0)return {x,y,width,height};
  }
  return undefined;
}

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
  return page.crop?matches.filter(match=>match.rects.some(rect=>rectVisibleOnPage(rect,page))):matches;
}

interface SearchResult { matches: SearchMatch[]; searching: boolean; error: string | null; hasText: boolean }
interface SearchState extends SearchResult { adapter: FolioAdapter; pages: PagePlan[]; query: string }

export function useDocumentSearch(adapter: FolioAdapter, pages: PagePlan[], query: string, enabled: boolean): SearchResult {
  const fonts = useSyncExternalStore(subscribeFonts, fontRevision);
  const needle = normalizeSearchQuery(query);
  const [state, setState] = useState<SearchState | null>(null);
  useEffect(() => {
    if (!enabled || !needle) return;
    let cancelled = false;
    let activePreparation: ReturnType<typeof acquireShapedText> | undefined;
    const timer = setTimeout(() => {
      void (async () => {
        const matches: SearchMatch[] = [];
        let hasText = false;
        const errors: string[] = [];
        const restricted=new Map<string,Promise<PageSearchResult>>();
        for (let index = 0; index < pages.length; index++) {
          if (cancelled) return;
          const page = pages[index];
          let text: PageText = { characters: [] };
          try {
            if(page.protection?.permissions.copyContent===false) {
              if(!adapter.searchPage)throw new Error("Search for this protected PDF requires the desktop search service.");
              const key=JSON.stringify([page.sourceId,page.pageIndex]);
              let request=restricted.get(key);
              if(!request){request=adapter.searchPage(page.sourceId,page.pageIndex,needle);restricted.set(key,request);}
              const found=await request;if(cancelled)return;
              matches.push(...found.matches.map((rects,index)=>({id:`${page.id}:source:${index}`,pageId:page.id,rects})));
              hasText ||= found.hasText;
              if(found.truncated)errors.push(`Page ${index+1}: search result limit reached. Refine the query to find more specific text.`);
            } else text = await readSearchPageText(adapter, page.sourceId, page.pageIndex);
          }
          catch (error) { errors.push(`Page ${index + 1}: ${error instanceof Error ? error.message : String(error)}`); }
          if (cancelled) return;
          matches.push(...findTextMatches(page.id, "source", text, needle));
          for (const overlay of page.overlays) {
            if (overlay.type !== "text") continue;
            if (overlay.shaping) {
              const lease = acquireShapedText(overlay);
              activePreparation = lease;
              try {
                const prepared = await lease.promise;
                if (cancelled) return;
                const text = {intrinsicRotation: overlay.rotation ?? 0, characters: prepared.characters.map(character=>({text:character.text,...rotateTextRect(overlay,character)}))};
                matches.push(...findTextMatches(page.id, `overlay:${overlay.id}`, text, needle));
              } catch (error) { errors.push(`Page ${index+1}: ${String(error)}`); }
              finally { lease.release(); if(activePreparation === lease) activePreparation=undefined; }
            } else matches.push(...findTextMatches(page.id, `overlay:${overlay.id}`, textOverlayCharacters(overlay), needle));
            if (cancelled) return;
          }
          hasText ||= text.characters.some((character) => !!character.text.trim()) || page.overlays.some((overlay) => overlay.type === "text" && !!overlay.text.trim());
          setState({ adapter, pages, query: needle, matches: matches.filter(match=>{const owner=pages.find(page=>page.id===match.pageId);return !owner?.crop||match.rects.some(rect=>rectVisibleOnPage(rect,owner));}), hasText, error: errors.length ? `Some pages could not be searched. ${errors[0]}` : null, searching: index < pages.length - 1 });
          // Let rendering and input run even when every extraction is cached.
          if (index < pages.length - 1) await new Promise<void>((resolve) => setTimeout(resolve, 0));
        }
        if (!pages.length && !cancelled) setState({ adapter, pages, query: needle, matches: [], hasText: false, error: null, searching: false });
      })();
    }, 120);
    return () => { cancelled = true; clearTimeout(timer); activePreparation?.release(); };
  }, [adapter, pages, needle, enabled, fonts]);

  if (!enabled || !needle) return { matches: [], searching: false, error: null, hasText: false };
  if (!state || state.adapter !== adapter || state.pages !== pages || state.query !== needle) return { matches: [], searching: true, error: null, hasText: false };
  return state;
}
