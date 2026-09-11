import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { FontPicker } from "../src/components/FontPicker";
import { drainFontAcquisitions, fontApi, fontFamily, retainCustomFonts, releaseUnownedFont } from "../src/editor/custom-fonts";
const id="b".repeat(64),info={id,name:"Local Serif",weight:400,italic:false,coverage:[[32,126]] as [number,number][]};
const secondId="c".repeat(64),secondInfo={...info,id:secondId,name:"Local Sans",weight:700};
function deferred<T>() { let resolve!:(value:T)=>void; const promise=new Promise<T>(done=>resolve=done); return {promise,resolve}; }
beforeEach(()=>{
  vi.stubGlobal("FontFace",class{load(){return Promise.resolve(this);}});
  Object.defineProperty(document,"fonts",{configurable:true,value:{add:vi.fn(),delete:vi.fn()}});
  vi.spyOn(fontApi,"listInstalled").mockResolvedValue([{id:"installed",name:"Local Serif",supported:true},{id:"second",name:"Local Sans",supported:true},{id:"blocked",name:"Restricted Font",supported:false,reason:"Embedding is restricted"}]);
  vi.spyOn(fontApi,"loadInstalled").mockResolvedValue(info);
  vi.spyOn(fontApi,"importFont").mockResolvedValue(info);
  vi.spyOn(fontApi,"info").mockResolvedValue(info);
  vi.spyOn(fontApi,"bytes").mockResolvedValue(new Uint8Array([1,2]).buffer);
  vi.spyOn(fontApi,"release").mockResolvedValue();
});
afterEach(async()=>{cleanup();retainCustomFonts(new Set());await Promise.all([releaseUnownedFont(id),releaseUnownedFont(secondId)]);vi.restoreAllMocks();vi.unstubAllGlobals();});
it("searches installed fonts and explains unsupported choices",async()=>{
  render(<FontPicker text="Hello" onChoose={vi.fn()} onClose={vi.fn()}/>);
  await screen.findByText("Local Serif");
  expect(screen.getByRole("button",{name:/Restricted Font/})).toBeDisabled();
  expect(screen.getByText("Embedding is restricted")).toBeVisible();
  fireEvent.change(screen.getByRole("searchbox",{name:"Search fonts"}),{target:{value:"local"}});
  expect(screen.queryByText("Restricted Font")).not.toBeInTheDocument();
});
it("previews current text in the loaded face and applies only on explicit confirmation",async()=>{
  const choose=vi.fn();render(<FontPicker text="Hello" onChoose={choose} onClose={vi.fn()}/>);
  expect(screen.getByRole("button",{name:"Apply font"})).toBeDisabled();
  fireEvent.click(await screen.findByRole("button",{name:"Local Serif"}));
  const preview=await screen.findByLabelText("Font preview");
  expect(preview).toHaveTextContent("Hello");
  expect(preview).toHaveStyle({fontFamily:fontFamily(id),fontKerning:"none",fontVariantLigatures:"none"});
  expect(choose).not.toHaveBeenCalled();
  expect(document.fonts.add).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button",{name:"Apply font"}));
  await waitFor(()=>expect(choose).toHaveBeenCalledWith(info));
  fireEvent.click(screen.getByRole("button",{name:"Apply font"}));
  expect(choose).toHaveBeenCalledOnce();
  expect(fontApi.release).not.toHaveBeenCalled();
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
it("loads no font bytes just to show the installed catalog",async()=>{
  render(<FontPicker text="Hello" onChoose={vi.fn()} onClose={vi.fn()}/>);
  await screen.findByRole("button",{name:"Local Serif"});
  expect(fontApi.bytes).not.toHaveBeenCalled();
  expect(fontApi.loadInstalled).not.toHaveBeenCalled();
});
it("releases the previous preview when switching and the last preview when canceled",async()=>{
  vi.mocked(fontApi.loadInstalled).mockImplementation(async value=>value==="second"?secondInfo:info);
  const close=vi.fn();render(<FontPicker text="Hello" onChoose={vi.fn()} onClose={close}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Local Serif"}));
  await screen.findByLabelText("Font preview");
  fireEvent.click(screen.getByRole("button",{name:"Local Sans"}));
  await waitFor(()=>expect(screen.getByLabelText("Font preview")).toHaveStyle({fontFamily:fontFamily(secondId)}));
  expect(fontApi.release).toHaveBeenCalledWith(id);
  fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
  await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(secondId));
  expect(close).toHaveBeenCalledOnce();
});
it("allows cancel while a native font load is pending and cleans up its late result",async()=>{
  const pending=deferred<typeof info>();vi.mocked(fontApi.loadInstalled).mockReturnValue(pending.promise);
  const close=vi.fn(),choose=vi.fn();render(<FontPicker text="Hello" onChoose={choose} onClose={close}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Local Serif"}));
  await waitFor(()=>expect(fontApi.loadInstalled).toHaveBeenCalled());
  fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
  expect(close).toHaveBeenCalledOnce();
  await act(async()=>pending.resolve(info));
  await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(id));
  expect(fontApi.bytes).not.toHaveBeenCalled();
  expect(choose).not.toHaveBeenCalled();
});
it("uses the latest requested preview without overlapping native loads",async()=>{
  const pending=deferred<typeof info>();
  vi.mocked(fontApi.loadInstalled).mockImplementation(value=>value==="installed"?pending.promise:Promise.resolve(secondInfo));
  render(<FontPicker text="Hello" onChoose={vi.fn()} onClose={vi.fn()}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Local Serif"}));
  await waitFor(()=>expect(fontApi.loadInstalled).toHaveBeenCalledWith("installed"));
  fireEvent.click(screen.getByRole("button",{name:"Local Sans"}));
  expect(fontApi.loadInstalled).toHaveBeenCalledTimes(1);
  await act(async()=>pending.resolve(info));
  await waitFor(()=>expect(screen.getByLabelText("Font preview")).toHaveStyle({fontFamily:fontFamily(secondId)}));
  expect(fontApi.release).toHaveBeenCalledWith(id);
  expect(fontApi.bytes).toHaveBeenCalledTimes(1);
  expect(fontApi.bytes).toHaveBeenCalledWith(secondId);
});
it("cleans up a pending FontFace immediately on unmount and ignores its completion",async()=>{
  const face=deferred<FontFace>();vi.stubGlobal("FontFace",class{load(){return face.promise;}});
  const choose=vi.fn(),view=render(<FontPicker text="Hello" onChoose={choose} onClose={vi.fn()}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Local Serif"}));
  await waitFor(()=>expect(fontApi.bytes).toHaveBeenCalledWith(id));
  view.unmount();
  await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(id));
  await act(async()=>face.resolve({} as FontFace));
  expect(document.fonts.add).not.toHaveBeenCalled();
  expect(choose).not.toHaveBeenCalled();
});
it("preserves a preview font retained by active documents or history",async()=>{
  retainCustomFonts(new Set([id]));
  render(<FontPicker text="Hello" onChoose={vi.fn()} onClose={vi.fn()}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Local Serif"}));
  await screen.findByLabelText("Font preview");
  fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
  expect(fontApi.release).not.toHaveBeenCalled();
  expect(document.fonts.delete).not.toHaveBeenCalled();
});
it("supports a supplied current font and substitute-dialog wording",async()=>{
  retainCustomFonts(new Set([id]));
  render(<FontPicker title="Choose a substitute font" description="Preview a replacement for this text." currentFontId={id} text="Hello" onChoose={vi.fn()} onClose={vi.fn()}/>);
  expect(screen.getByRole("dialog",{name:"Choose a substitute font"})).toHaveAccessibleDescription("Preview a replacement for this text.");
  await screen.findByLabelText("Font preview");
  expect(fontApi.info).toHaveBeenCalledWith(id);
  expect(fontApi.loadInstalled).not.toHaveBeenCalled();
});
it("transfers the preview before a parent synchronously unmounts the dialog",async()=>{
  const choose=vi.fn(()=>view.unmount());
  const view=render(<FontPicker text="Hello" onChoose={choose} onClose={vi.fn()}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Local Serif"}));
  await screen.findByLabelText("Font preview");
  fireEvent.click(screen.getByRole("button",{name:"Apply font"}));
  expect(choose).toHaveBeenCalledWith(info);
  expect(fontApi.release).not.toHaveBeenCalled();
});
it("revalidates changed text before allowing Apply",async()=>{
  const choose=vi.fn(),close=vi.fn();
  const view=render(<FontPicker text="Hello" onChoose={choose} onClose={close}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Local Serif"}));
  await screen.findByLabelText("Font preview");
  view.rerender(<FontPicker text="🙂" onChoose={choose} onClose={close}/>);
  expect(screen.getByRole("alert")).toHaveTextContent("U+1F642");
  expect(screen.getByRole("button",{name:"Apply font"})).toBeDisabled();
  expect(screen.queryByLabelText("Font preview")).not.toBeInTheDocument();
  expect(choose).not.toHaveBeenCalled();
});
it("cancels a new dialog immediately while an older acquisition is still pending",async()=>{
  const pending=deferred<typeof info>();
  vi.mocked(fontApi.loadInstalled).mockImplementationOnce(()=>pending.promise).mockResolvedValue(info);
  const old=render(<FontPicker text="Hello" onChoose={vi.fn()} onClose={vi.fn()}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Local Serif"}));
  await waitFor(()=>expect(fontApi.loadInstalled).toHaveBeenCalledTimes(1));
  old.unmount();
  const close=vi.fn(),choose=vi.fn();
  render(<FontPicker text="Hello" onChoose={choose} onClose={close}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Local Serif"}));
  fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
  expect(close).toHaveBeenCalledOnce();
  await act(async()=>pending.resolve(info));
  await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(id));
  expect(fontApi.loadInstalled).toHaveBeenCalledTimes(1);
  expect(fontApi.bytes).not.toHaveBeenCalled();
  expect(choose).not.toHaveBeenCalled();
});
it("finishes canceled native acquisition cleanup before a new dialog registers the same font",async()=>{
  const oldResponse=deferred<typeof info>(),newResponse=deferred<typeof info>();
  let registered=false,loads=0;
  const events:string[]=[];
  vi.mocked(fontApi.loadInstalled).mockImplementation(()=>{
    registered=true;
    events.push(`load${++loads}`);
    return loads===1?oldResponse.promise:newResponse.promise;
  });
  vi.mocked(fontApi.release).mockImplementation(async()=>{registered=false;events.push("release");});
  vi.mocked(fontApi.bytes).mockImplementation(async()=>{
    events.push("bytes");
    if(!registered) throw new Error("The requested font is not loaded.");
    return new Uint8Array([1,2]).buffer;
  });
  const old=render(<FontPicker text="Hello" onChoose={vi.fn()} onClose={vi.fn()}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Local Serif"}));
  await waitFor(()=>expect(fontApi.loadInstalled).toHaveBeenCalledTimes(1));
  old.unmount();
  render(<FontPicker text="Hello" onChoose={vi.fn()} onClose={vi.fn()}/>);
  fireEvent.click(await screen.findByRole("button",{name:"Local Serif"}));
  await act(async()=>{await Promise.resolve();});
  let drained=false;
  const drain=drainFontAcquisitions().then(()=>{drained=true;});
  // The first JS response arrives while the next font choice is pending.
  await act(async()=>oldResponse.resolve(info));
  await waitFor(()=>expect(fontApi.loadInstalled).toHaveBeenCalledTimes(2));
  expect(drained).toBe(false);
  await act(async()=>newResponse.resolve(info));
  await screen.findByLabelText("Font preview");
  await drain;
  expect(drained).toBe(true);
  expect(events).toEqual(["load1","release","load2","bytes"]);
  expect(screen.getByRole("button",{name:"Apply font"})).toBeEnabled();
});
