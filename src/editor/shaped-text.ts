import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, useSyncExternalStore } from "react";
import { holdCustomFont } from "./custom-fonts";
import type { NativeTextPreview } from "./native-text-preview";
import type { AnnotationRect, PdfTextCharacter, TextOverlay } from "./types";

export interface PreparedTextOverlay {
  preview: NativeTextPreview;
  origin: [number, number];
  bounds: AnnotationRect;
  characters: PdfTextCharacter[];
}
export interface ShapedTextState { status: "loading" | "ready" | "error"; result?: PreparedTextOverlay; error?: string; retry?(): void }
export const shapedTextApi = { prepare: (overlay: TextOverlay) => invoke<PreparedTextOverlay>("prepare_text_overlay", { overlay }) };
interface Entry {
  key: string; overlay: TextOverlay; state: ShapedTextState; users: number; started: boolean;
  promise: Promise<PreparedTextOverlay>; resolve(value: PreparedTextOverlay): void; reject(error: Error): void;
  releaseFont(): Promise<void>; bytes: number;
}
const entries = new Map<string, Entry>();
const listeners = new Set<() => void>();
const retries = new Map<string, number>();
let revision = 0, running = false;
const MAX_ENTRIES = 64, MAX_BYTES = 32 * 1024 * 1024;
const loading: ShapedTextState = Object.freeze({status: "loading"});
export const subscribeShapedText = (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener); }; };
export const shapedTextRevision = () => revision;
function emit() { revision++; listeners.forEach(listener => listener()); }
export function shapedTextKey(overlay: TextOverlay) {
  return JSON.stringify([overlay.fontId, overlay.text, overlay.fontSize, overlay.color, overlay.shaping]);
}
export function shapedTextState(overlay: TextOverlay) { return overlay.shaping ? entries.get(shapedTextKey(overlay))?.state : undefined; }
function drop(entry: Entry) {
  if (entries.get(entry.key) === entry) entries.delete(entry.key);
  if (!entry.started) { entry.reject(new Error("Text preparation canceled.")); void entry.releaseFont(); }
}
function prune() {
  let bytes = [...entries.values()].reduce((total, entry) => total + entry.bytes, 0);
  for (const entry of entries.values()) {
    if (entries.size < MAX_ENTRIES && bytes <= MAX_BYTES) break;
    if (entry.users === 0 && entry.state.status !== "loading") { bytes -= entry.bytes; drop(entry); }
  }
}
async function processQueue() {
  if (running) return;
  running = true;
  try {
    for (;;) {
      const entry = [...entries.values()].find(value => !value.started && value.users > 0);
      if (!entry) break;
      entry.started = true;
      try {
        const result = await shapedTextApi.prepare(entry.overlay);
        entry.bytes = JSON.stringify(result).length * 2;
        if (result.preview.version !== 1 || result.preview.text !== entry.overlay.text || result.preview.fontId !== entry.overlay.fontId)
          throw new Error("Native text result does not match the requested text and font.");
        let otherBytes = [...entries.values()].reduce((total, other) => total + (other === entry ? 0 : other.bytes), 0);
        for (const other of entries.values()) {
          if (otherBytes + entry.bytes <= MAX_BYTES) break;
          if (other !== entry && !other.users && other.state.status !== "loading") { otherBytes -= other.bytes; drop(other); }
        }
        if (entry.bytes > MAX_BYTES || otherBytes + entry.bytes > MAX_BYTES) throw new Error("Text preview memory limit reached. Close unused documents and try again.");
        entry.state = {status: "ready", result};
        entry.resolve(result);
      } catch (error) {
        entry.bytes = 0;
        const message = error instanceof Error ? error.message : String(error);
        entry.state = {status: "error", error: message}; entry.reject(new Error(message));
      } finally {
        await entry.releaseFont();
        if (!entry.users) drop(entry);
        prune(); emit();
      }
    }
  } finally { running = false; }
}
export function acquireShapedText(overlay: TextOverlay): {promise: Promise<PreparedTextOverlay>; release(): void} {
  const key = shapedTextKey(overlay);
  let entry = entries.get(key);
  if (!entry) {
    prune();
    if (entries.size >= MAX_ENTRIES || !overlay.fontId || !overlay.shaping) {
      const promise = Promise.reject<PreparedTextOverlay>(new Error(!overlay.fontId ? "Choose a custom font for shaped text." : "Text preparation limit reached."));
      void promise.catch(() => {});
      return {promise, release() {}};
    }
    let resolve!: Entry["resolve"], reject!: Entry["reject"];
    const promise = new Promise<PreparedTextOverlay>((ok, fail) => { resolve = ok; reject = fail; });
    void promise.catch(() => {});
    entry = {key, overlay: {...overlay, id: "preview", x: 0, y: 0, rotation: 0, shaping: {...overlay.shaping}}, state: loading,
      users: 0, started: false, bytes: 0, promise, resolve, reject, releaseFont: holdCustomFont(overlay.fontId)};
    entries.set(key, entry);
  }
  entries.delete(key); entries.set(key, entry); entry.users++;
  queueMicrotask(() => { void processQueue(); });
  let active = true;
  return {promise: entry.promise, release() {
    if (!active) return; active = false; entry!.users--;
    if (!entry!.users && entry!.state.status === "loading" && !entry!.started) drop(entry!);
    prune();
  }};
}
export function retryShapedText(overlay: TextOverlay) {
  const key=shapedTextKey(overlay),entry=entries.get(key);
  if (entry?.state.status === "loading") return;
  if (entry) drop(entry);
  retries.set(key,(retries.get(key)??0)+1);
  if(retries.size>MAX_ENTRIES) retries.delete(retries.keys().next().value!);
  emit();
}
export function clearShapedTextCache() {
  for (const entry of entries.values()) drop(entry);
  emit();
}
export function useShapedText(overlay?: TextOverlay): ShapedTextState | undefined {
  useSyncExternalStore(subscribeShapedText, shapedTextRevision);
  const key = overlay?.shaping ? shapedTextKey(overlay) : undefined;
  // Errors from admission limits are local to the consumer; ordinary errors are cached.
  const [failure, setFailure] = useState<{key: string; error: string}>();
  const retry = key ? retries.get(key) ?? 0 : 0;
  useEffect(() => {
    if (!key || !overlay) return;
    let active = true;
    const lease = acquireShapedText(overlay);
    lease.promise.catch(error => { if (active) setFailure({key, error: String(error instanceof Error ? error.message : error)}); });
    return () => { active = false; lease.release(); };
  }, [key, retry]);
  if (!key || !overlay) return undefined;
  const state = shapedTextState(overlay) ?? (failure?.key === key ? {status: "error" as const, error: failure.error} : loading);
  return state.status === "error" ? {...state,retry:()=>retryShapedText(overlay)} : state;
}
