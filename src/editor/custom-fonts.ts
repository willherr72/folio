import { invoke } from "@tauri-apps/api/core";

export interface FontInfo { id: string; name: string; weight: number; italic: boolean; coverage: [number, number][] }
export interface InstalledFont { id: string; name: string; supported: boolean; reason?: string | null }
export const fontApi = {
  listInstalled: () => invoke<InstalledFont[]>("list_installed_fonts"),
  loadInstalled: (id: string) => invoke<FontInfo>("load_installed_font", { id }),
  importFont: () => invoke<FontInfo | null>("import_font"),
  info: (id: string) => invoke<FontInfo>("font_info", { id }),
  bytes: (id: string) => invoke<ArrayBuffer | number[]>("font_bytes", { id }),
  release: (id: string) => invoke<void>("release_font", { id }),
};
export interface FontState { status: "loading" | "ready" | "error"; family: string; info?: FontInfo; error?: string }
interface Entry { state: FontState; promise: Promise<FontInfo>; face?: FontFace }
const entries = new Map<string, Entry>();
let retained = new Set<string>();
const leases = new Map<string, number>();
const listeners = new Set<() => void>();
let revision = 0;
const releasing = new Set<Promise<void>>();
function emit() { revision++; listeners.forEach(listener => listener()); }
export const subscribeFonts = (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener); }; };
export const fontRevision = () => revision;
export const customFontState = (id: string) => entries.get(id)?.state;
export const fontFamily = (id: string) => `FolioFont_${id}`;
export async function flushFontReleases() { await Promise.allSettled([...releasing]); }

// Native registration happens before its JS response arrives. Serialize the
// whole acquisition across dialogs so an older canceled response cannot delete
// a newer registration before that dialog has installed its preview lease.
let acquisitionQueue: Promise<void> = Promise.resolve();
export function acquireFontInOrder(work: () => Promise<void>) {
  const result = acquisitionQueue.then(work, work);
  acquisitionQueue = result.catch(() => {});
  return result;
}
export async function drainFontAcquisitions() {
  let pending: Promise<void>;
  do {
    pending = acquisitionQueue;
    await pending;
  } while (pending !== acquisitionQueue);
}

export function ensureCustomFont(id: string, knownInfo?: FontInfo): Promise<FontInfo> {
  if (!/^[a-f0-9]{64}$/.test(id)) return Promise.reject(new Error("Invalid font resource identifier."));
  const existing = entries.get(id);
  if (existing && existing.state.status !== "error") return existing.promise;
  const entry: Entry = { state: { status: "loading", family: fontFamily(id) }, promise: undefined! };
  entries.set(id, entry);
  entry.promise = (async () => {
    try {
      const [info, payload] = await Promise.all([knownInfo ?? fontApi.info(id), fontApi.bytes(id)]);
      if (entries.get(id) !== entry) throw new Error("Font loading was cancelled.");
      const bytes = payload instanceof ArrayBuffer ? payload : new Uint8Array(payload).buffer;
      // Each face has a private family; intrinsic bold/italic outlines need no synthetic styling.
      const face = await new FontFace(entry.state.family, bytes, { style: "normal", weight: "400" }).load();
      if (entries.get(id) !== entry) throw new Error("Font loading was cancelled.");
      document.fonts.add(face);
      entry.face = face;
      entry.state = { ...entry.state, status: "ready", info };
      emit();
      return info;
    } catch (error) {
      if (entries.get(id) === entry) {
        entry.state = { ...entry.state, status: "error", error: error instanceof Error ? error.message : String(error) };
        emit();
      }
      throw error;
    }
  })();
  return entry.promise;
}

export function releaseUnownedFont(id: string): Promise<void> {
  if (retained.has(id) || leases.has(id)) return Promise.resolve();
  const entry = entries.get(id);
  entries.delete(id);
  if (entry?.face) document.fonts.delete(entry.face);
  if (entry) emit();
  const work = fontApi.release(id).catch(() => { /* Native registry also has a fixed memory limit. */ });
  releasing.add(work);
  void work.finally(() => releasing.delete(work));
  return work;
}
export function retainCustomFonts(ids: Set<string>) {
  const previous = retained;
  retained = new Set(ids);
  for (const id of previous) if (!retained.has(id)) void releaseUnownedFont(id);
}

export function customTextError(info: FontInfo, text: string): string | null {
  for (const character of text) {
    if (character === "\n") continue;
    const value = character.codePointAt(0)!;
    let low = 0, high = info.coverage.length - 1, found = false;
    while (low <= high) {
      const middle = (low + high) >> 1, [start, end] = info.coverage[middle];
      if (value < start) high = middle - 1;
      else if (value > end) low = middle + 1;
      else { found = true; break; }
    }
    if (!found) return `Folio cannot use “${character}” (U+${value.toString(16).toUpperCase().padStart(4,"0")}) with ${info.name}. Choose another font or change this character.`;
  }
  return null;
}

/** Keep a dialog preview alive independently of committed workspace history. */
export function holdCustomFont(id: string): (releaseIfUnowned?: boolean) => Promise<void> {
  leases.set(id, (leases.get(id) ?? 0) + 1);
  let active = true;
  return (releaseIfUnowned = true) => {
    if (!active) return Promise.resolve();
    active = false;
    const count = leases.get(id) ?? 1;
    if (count > 1) leases.set(id, count - 1); else leases.delete(id);
    return releaseIfUnowned ? releaseUnownedFont(id) : Promise.resolve();
  };
}
