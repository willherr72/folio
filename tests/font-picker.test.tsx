import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { FontPicker } from "../src/components/FontPicker";
import { fontApi, retainCustomFonts, releaseUnownedFont } from "../src/editor/custom-fonts";
const id="b".repeat(64),info={id,name:"Local Serif",weight:400,italic:false,coverage:[[32,126]] as [number,number][]};
beforeEach(()=>{
  vi.stubGlobal("FontFace",class{load(){return Promise.resolve(this);}});
  Object.defineProperty(document,"fonts",{configurable:true,value:{add:vi.fn(),delete:vi.fn()}});
  vi.spyOn(fontApi,"listInstalled").mockResolvedValue([{id:"installed",name:"Local Serif",supported:true},{id:"blocked",name:"Restricted Font",supported:false,reason:"Embedding is restricted"}]);
  vi.spyOn(fontApi,"loadInstalled").mockResolvedValue(info);
  vi.spyOn(fontApi,"importFont").mockResolvedValue(info);
  vi.spyOn(fontApi,"bytes").mockResolvedValue(new Uint8Array([1,2]).buffer);
  vi.spyOn(fontApi,"release").mockResolvedValue();
});
afterEach(async()=>{cleanup();retainCustomFonts(new Set());await releaseUnownedFont(id);vi.restoreAllMocks();vi.unstubAllGlobals();});
it("searches installed fonts and explains unsupported choices",async()=>{
  render(<FontPicker text="Hello" onChoose={vi.fn()} onClose={vi.fn()}/>);
  await screen.findByText("Local Serif");
  expect(screen.getByRole("button",{name:/Restricted Font/})).toBeDisabled();
  expect(screen.getByText("Embedding is restricted")).toBeVisible();
  fireEvent.change(screen.getByRole("searchbox",{name:"Search fonts"}),{target:{value:"local"}});
  expect(screen.queryByText("Restricted Font")).not.toBeInTheDocument();
});
it("loads and validates a selected font before applying it",async()=>{
  const choose=vi.fn();render(<FontPicker text="Hello" onChoose={choose} onClose={vi.fn()}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Local Serif"}));
  await waitFor(()=>expect(choose).toHaveBeenCalledWith(info));
  expect(document.fonts.add).toHaveBeenCalledOnce();
});
it("keeps the picker open and releases an imported font that cannot represent the current text",async()=>{
  const choose=vi.fn();render(<FontPicker text="Missing 🙂" onChoose={choose} onClose={vi.fn()}/>);
  fireEvent.click(screen.getByRole("button",{name:"Import font…"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("U+1F642");
  expect(choose).not.toHaveBeenCalled();
  expect(fontApi.release).toHaveBeenCalledWith(id);
});
it("releases a late import when its dialog unmounts",async()=>{
  let resolve!:(value:typeof info)=>void;vi.mocked(fontApi.importFont).mockImplementation(()=>new Promise(done=>resolve=done));
  const choose=vi.fn(),view=render(<FontPicker text="Hello" onChoose={choose} onClose={vi.fn()}/>);
  fireEvent.click(screen.getByRole("button",{name:"Import font…"}));
  await waitFor(()=>expect(fontApi.importFont).toHaveBeenCalled());view.unmount();resolve(info);
  await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(id));expect(choose).not.toHaveBeenCalled();
});
