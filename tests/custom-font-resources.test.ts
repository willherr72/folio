import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { fontApi, ensureCustomFont, customFontState, retainCustomFonts, releaseUnownedFont, customTextError } from "../src/editor/custom-fonts";
const id = "a".repeat(64);
const info = { id, name: "Example Serif Bold", weight: 700, italic: false, coverage: [[32,126],[233,233]] as [number,number][] };
const added = new Set<FontFace>();
beforeEach(() => {
  vi.stubGlobal("FontFace", class { family: string; constructor(family: string, public bytes: ArrayBuffer) { this.family=family; } load() { return Promise.resolve(this); } });
  Object.defineProperty(document, "fonts", { configurable:true, value:{add:vi.fn((font:FontFace)=>added.add(font)),delete:vi.fn((font:FontFace)=>added.delete(font))} });
  vi.spyOn(fontApi,"info").mockResolvedValue(info);
  vi.spyOn(fontApi,"bytes").mockResolvedValue(new Uint8Array([0,1,2,3]).buffer);
  vi.spyOn(fontApi,"release").mockResolvedValue();
});
afterEach(async()=>{retainCustomFonts(new Set());await releaseUnownedFont(id);added.clear();vi.restoreAllMocks();vi.unstubAllGlobals();});
it("loads exact font bytes once and retains the face while any history owns it",async()=>{
  retainCustomFonts(new Set([id]));
  await Promise.all([ensureCustomFont(id),ensureCustomFont(id)]);
  expect(fontApi.bytes).toHaveBeenCalledOnce();
  expect(added.size).toBe(1);
  expect(customFontState(id)?.status).toBe("ready");
  await releaseUnownedFont(id);
  expect(fontApi.release).not.toHaveBeenCalled();
  retainCustomFonts(new Set());
  await vi.waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(id));
  expect(added.size).toBe(0);
});
it("reports a missing glyph without silently using a fallback font",()=>{
  expect(customTextError(info,"café\nsecond line")).toBeNull();
  expect(customTextError(info,"missing 🙂")).toContain("U+1F642");
});
it("does not install a font face after its last owner disappears during loading",async()=>{
  let resolve!:(bytes:ArrayBuffer)=>void;
  vi.mocked(fontApi.bytes).mockImplementation(()=>new Promise(done=>resolve=done));
  retainCustomFonts(new Set([id]));
  const loading=ensureCustomFont(id);
  retainCustomFonts(new Set());
  resolve(new Uint8Array([1,2]).buffer);
  await loading.catch(()=>{});
  expect(added.size).toBe(0);
  expect(customFontState(id)).toBeUndefined();
});
