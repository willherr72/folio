import type { InkPoint } from "./types";

export interface SavedSignature { id: string; name: string; paths: InkPoint[][] }
export const SIGNATURES_KEY = "folio.signatures.v1";
export const MAX_SIGNATURES = 30;
export const MAX_SIGNATURE_POINTS = 10_000;
const MAX_STORAGE_LENGTH = 2_000_000;
const MAX_PATHS = 512;

type RecordValue = Record<string, unknown>;
const isRecord = (value: unknown): value is RecordValue => !!value && typeof value === "object" && !Array.isArray(value);

function signatureName(value: unknown): string {
  if (typeof value !== "string" || !value.trim() || value.trim().length > 80 || /[\u0000-\u001f\u007f]/.test(value)) {
    throw new Error("Give the signature a name between 1 and 80 characters.");
  }
  return value.trim();
}

function signaturePaths(value: unknown): InkPoint[][] {
  if (!Array.isArray(value) || value.length === 0 || value.length > MAX_PATHS) {
    throw new Error("Draw a signature with no more than 512 strokes.");
  }
  let count = 0;
  return value.map(path => {
    if (!Array.isArray(path) || path.length < 2 || (count += path.length) > MAX_SIGNATURE_POINTS) {
      throw new Error("The signature must contain between 2 and 10,000 drawing points.");
    }
    return path.map(point => {
      if (!isRecord(point) || typeof point.x !== "number" || typeof point.y !== "number"
        || !Number.isFinite(point.x) || !Number.isFinite(point.y)
        || point.x < 0 || point.x > 360 || point.y < 0 || point.y > 140) {
        throw new Error("The signature contains invalid drawing coordinates.");
      }
      return { x: point.x, y: point.y };
    });
  });
}

/** Reads afresh on every operation so another dialog's edits are not overwritten. */
export function loadSignatures(): SavedSignature[] {
  let text: string | null;
  try { text = localStorage.getItem(SIGNATURES_KEY); }
  catch { throw new Error("Saved signatures could not be read from local storage."); }
  if (text === null) return [];
  try {
    if (text.length > MAX_STORAGE_LENGTH) throw new Error("too large");
    const value: unknown = JSON.parse(text);
    if (!isRecord(value) || value.version !== 1 || !Array.isArray(value.signatures) || value.signatures.length > MAX_SIGNATURES) {
      throw new Error("unsupported library");
    }
    const ids = new Set<string>();
    const names = new Set<string>();
    return value.signatures.map(item => {
      if (!isRecord(item) || typeof item.id !== "string" || !item.id || item.id.length > 100 || ids.has(item.id)) {
        throw new Error("invalid signature identity");
      }
      const name = signatureName(item.name);
      if (names.has(name.toLowerCase())) throw new Error("duplicate name");
      ids.add(item.id); names.add(name.toLowerCase());
      return { id: item.id, name, paths: signaturePaths(item.paths) };
    });
  } catch {
    throw new Error("The saved signature library is damaged or uses an unsupported format. You can still draw a signature, or reset the saved library.");
  }
}

function persist(signatures: SavedSignature[]): SavedSignature[] {
  const text = JSON.stringify({ version: 1, signatures });
  if (text.length > MAX_STORAGE_LENGTH) throw new Error("The signature library is full. Delete a saved signature before adding another.");
  try { localStorage.setItem(SIGNATURES_KEY, text); }
  catch { throw new Error("The signature library could not be saved. Local storage may be full or unavailable."); }
  return signatures;
}

function ensureUniqueName(library: SavedSignature[], name: string, exceptId?: string) {
  if (library.some(item => item.id !== exceptId && item.name.toLowerCase() === name.toLowerCase())) {
    throw new Error("A saved signature already uses that name. Choose another name.");
  }
}

export function saveSignature(name: string, paths: InkPoint[][]): SavedSignature[] {
  const library = loadSignatures();
  if (library.length >= MAX_SIGNATURES) throw new Error("You can save up to 30 signatures. Delete one before adding another.");
  const normalizedName = signatureName(name);
  ensureUniqueName(library, normalizedName);
  return persist([...library, { id: crypto.randomUUID(), name: normalizedName, paths: signaturePaths(paths) }]);
}

export function renameSignature(id: string, name: string): SavedSignature[] {
  const library = loadSignatures();
  if (!library.some(item => item.id === id)) throw new Error("This saved signature is no longer available. Reopen the signature library.");
  const normalizedName = signatureName(name);
  ensureUniqueName(library, normalizedName, id);
  return persist(library.map(item => item.id === id ? { ...item, name: normalizedName } : item));
}

export function deleteSignature(id: string): SavedSignature[] {
  const library = loadSignatures();
  if (!library.some(item => item.id === id)) throw new Error("This saved signature is no longer available. Reopen the signature library.");
  return persist(library.filter(item => item.id !== id));
}

export function resetSignatureLibrary(): void {
  try { localStorage.removeItem(SIGNATURES_KEY); }
  catch { throw new Error("The saved signature library could not be reset. Local storage is unavailable."); }
}
