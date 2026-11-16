import type { FolioAdapter } from './adapter';

interface Entry {
 adapter: FolioAdapter; provider: NonNullable<FolioAdapter['getPageReadingText']>;
 sourceId: string; request: Promise<string>; units: number;
}
const entries = new Map<string, Entry>();
const MAX_PAGES = 60, MAX_UNITS = 250_000;
function prune() {
 let units = Array.from(entries.values()).reduce((sum, entry) => sum + entry.units, 0);
 while(entries.size > MAX_PAGES || units > MAX_UNITS) {
  const key = entries.keys().next().value;
  if(key === undefined) break;
  units -= entries.get(key)!.units;
  entries.delete(key);
 }
}
export function clearPageReadingTextCache(sourceIds: string[]) {
 const sources = new Set(sourceIds);
 for(const [key, entry] of entries) if(sources.has(entry.sourceId)) entries.delete(key);
}
/** Exact reading text only. Rich geometry retains its separate existing cache. */
export function readPageReadingText(adapter: FolioAdapter, sourceId: string, pageIndex: number): Promise<string> {
 const provider = adapter.getPageReadingText;
 if(!provider) return Promise.resolve('');
 const key = JSON.stringify([sourceId, pageIndex]);
 let entry = entries.get(key);
 if(entry?.adapter !== adapter || entry?.provider !== provider) entry = undefined;
 if(!entry) {
  const next: Entry = {adapter, provider, sourceId, units: 0, request: Promise.resolve('')};
  next.request = Promise.resolve().then(() => provider.call(adapter, sourceId, pageIndex)).then(text => {
   // Cleared, superseded or evicted requests must never repopulate ownership.
   if(entries.get(key) === next) {next.units = text.length; prune();}
   return text;
  }).catch((error: unknown) => {
   if(entries.get(key) === next) entries.delete(key);
   throw error;
  });
  entry = next;
 }
 entries.delete(key); entries.set(key, entry); prune();
 return entry.request;
}
