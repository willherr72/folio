import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { FontPicker } from "../src/components/FontPicker";
import { fontApi, retainCustomFonts, releaseUnownedFont } from "../src/editor/custom-fonts";
import { shapedTextApi } from "../src/editor/shaped-text";
const id = "e".repeat(64);
const info = { id, name: "Arabic face", weight: 400, italic: false, coverage: [[32,126]] as [number,number][], shapedCoverage: [[32,126],[0x600,0x6ff]] as [number,number][] };
const shaping = { version: 1, direction: "auto", ligatures: true } as const;
function prepared(text: string) {
  return { preview: { version: 1 as const, fontId: id, text, width: 100, height: 100, rotation: 0 as const, color: [0,0,0] as [number,number,number], outlines: [{ glyphId: 3, path: "M0 0L5 0L5 10Z" }], glyphs: [{ glyphId: 3, transform: [1,0,0,1,0,0] as [number,number,number,number,number,number] }] }, origin: [0,0] as [number,number], bounds: { x: 0, y: 0, width: 20, height: 24 }, characters: [] };
}
beforeEach(() => {
  vi.stubGlobal("FontFace", class { load() { return Promise.resolve(this); } });
  Object.defineProperty(document,"fonts", { configurable: true, value: { add: vi.fn(), delete: vi.fn() } });
  vi.spyOn(fontApi,"listInstalled").mockResolvedValue([{ id: "arabic", name: "Arabic face", supported: true }]);
  vi.spyOn(fontApi,"loadInstalled").mockResolvedValue(info);
  vi.spyOn(fontApi,"bytes").mockResolvedValue(new Uint8Array([1,2]).buffer);
  vi.spyOn(fontApi,"release").mockResolvedValue();
});
afterEach(async () => { cleanup(); retainCustomFonts(new Set()); await releaseUnownedFont(id); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
it("waits for native shaping before allowing a custom font and previews exact outlines", async () => {
  let resolve!: (result: ReturnType<typeof prepared>) => void;
  vi.spyOn(shapedTextApi,"prepare").mockImplementation(() => new Promise(done => { resolve = done; }));
  const choose = vi.fn();
  render(<FontPicker text="العربية" shaping={shaping} fontSize={42} onChoose={choose} onClose={() => {}}/>);
  fireEvent.click(await screen.findByRole("button", { name: "Arabic face" }));
  await waitFor(() => expect(shapedTextApi.prepare).toHaveBeenCalledWith(expect.objectContaining({ fontSize: 42 })));
  expect(screen.getByRole("button", { name: "Apply font" })).toBeDisabled();
  expect(screen.queryByLabelText("Font preview")).toBeNull();
  await act(async () => resolve(prepared("العربية")));
  const preview = await screen.findByLabelText("Font preview");
  expect(preview.querySelector("path")).toHaveAttribute("d", "M0 0L5 0L5 10Z");
  fireEvent.click(screen.getByRole("button", { name: "Apply font" }));
  expect(choose).toHaveBeenCalledWith(info);
});
it("retains the text and blocks Apply when native shaping rejects a font", async () => {
  vi.spyOn(shapedTextApi,"prepare").mockRejectedValue(new Error("Mixed direction is unsupported"));
  const choose = vi.fn();
  render(<FontPicker text="a العربية" shaping={shaping} onChoose={choose} onClose={() => {}}/>);
  fireEvent.click(await screen.findByRole("button", { name: "Arabic face" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Mixed direction");
  expect(screen.getByRole("button", { name: "Apply font" })).toBeDisabled();
  expect(choose).not.toHaveBeenCalled();
});

it("retries a failed native preview without changing the chosen font", async()=>{
 const prepare=vi.spyOn(shapedTextApi,"prepare").mockRejectedValueOnce(new Error("Preview limit reached")).mockResolvedValue(prepared("سلام"));
 render(<FontPicker text="سلام" shaping={shaping} onChoose={()=>{}} onClose={()=>{}}/>);
 fireEvent.click(await screen.findByRole("button",{name:"Arabic face"}));
 await screen.findByRole("alert");
 fireEvent.click(screen.getByRole("button",{name:"Retry preview"}));
 await waitFor(()=>expect(screen.getByRole("button",{name:"Apply font"})).toBeEnabled());expect(prepare).toHaveBeenCalledTimes(2);
});
