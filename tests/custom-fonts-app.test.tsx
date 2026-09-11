import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { App } from "../src/App";
import { nativeAdapter } from "../src/editor/adapter";
import { fontApi, flushFontReleases, customFontState } from "../src/editor/custom-fonts";

const id = "b".repeat(64);
const info = { id, name: "Exact Serif Italic", weight: 400, italic: true, coverage: [[32,126],[233,233]] as [number,number][] };
vi.mock("../src/components/DocumentViewport", () => ({ DocumentViewport: (props: any) => <button onClick={() => props.onAddText(props.pages[0].id, {x: 40, y: 70})}>Place added text</button> }));
beforeEach(() => {
  localStorage.clear();
  vi.stubGlobal("FontFace", class { load() { return Promise.resolve(this); } });
  Object.defineProperty(document, "fonts", { configurable: true, value: { add: vi.fn(), delete: vi.fn() } });
  vi.spyOn(fontApi, "listInstalled").mockResolvedValue([{id:"installed",name:info.name,supported:true}]);
  vi.spyOn(fontApi, "loadInstalled").mockResolvedValue(info);
  vi.spyOn(fontApi, "info").mockResolvedValue(info);
  vi.spyOn(fontApi, "bytes").mockResolvedValue(new Uint8Array([0,1,2]).buffer);
  vi.spyOn(fontApi, "release").mockResolvedValue();
  vi.spyOn(nativeAdapter, "openPdf").mockResolvedValue({ id: "fonts", name: "Fonts.pdf", pages: [{ width: 500, height: 600 }] });
  vi.spyOn(nativeAdapter, "renderPage").mockResolvedValue("data:image/png;base64,");
  vi.spyOn(nativeAdapter, "exportPdf").mockResolvedValue("Saved.pdf");
  vi.spyOn(nativeAdapter, "closeDocument").mockResolvedValue();
});
afterEach(async () => { cleanup(); await act(async () => { await Promise.resolve(); await flushFontReleases(); }); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
async function openAndChoose() {
  render(<App initialDemo={false}/>);
  fireEvent.click(screen.getByRole("button", {name: "Open a PDF"}));
  await screen.findByRole("tab", {name: "Fonts.pdf"});
  fireEvent.click(screen.getByRole("button", {name: "Text"}));
  fireEvent.click(screen.getByRole("button", {name: "Place added text"}));
  fireEvent.change(screen.getByRole("textbox", {name: "Content"}), {target: {value: "A café"}});
  fireEvent.click(screen.getByRole("button", {name: "More fonts…"}));
  fireEvent.click(await screen.findByRole("button", {name: info.name}));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
}
it("exports custom font references and retains bytes through undo/redo until the tab closes", async () => {
  await openAndChoose();
  expect(screen.getByRole("combobox", {name: "Font"})).toHaveValue("custom");
  fireEvent.click(screen.getByRole("button", {name: "Undo"}));
  expect(screen.getByRole("combobox", {name: "Font"})).toHaveValue("Helvetica");
  expect(fontApi.release).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", {name: "Redo"}));
  expect(screen.getByRole("combobox", {name: "Font"})).toHaveValue("custom");
  fireEvent.click(screen.getByRole("button", {name: "Save a copy"}));
  await waitFor(() => expect(nativeAdapter.exportPdf).toHaveBeenCalledOnce());
  expect(vi.mocked(nativeAdapter.exportPdf).mock.calls[0][0][0].overlays[0]).toMatchObject({type: "text", text: "A café", fontId: id});
  fireEvent.click(screen.getByRole("button", {name: "Close Fonts.pdf"}));
  await waitFor(() => expect(fontApi.release).toHaveBeenCalledWith(id));
  expect(customFontState(id)).toBeUndefined();
});
it("keeps unsupported typed characters visible in the editor with a correction message", async () => {
  await openAndChoose();
  fireEvent.change(screen.getByRole("textbox", {name: "Content"}), {target: {value: "A 🙂"}});
  expect(screen.getByRole("textbox", {name: "Content"})).toHaveValue("A 🙂");
  expect(screen.getByRole("alert")).toHaveTextContent("U+1F642");
  fireEvent.change(screen.getByRole("combobox", {name: "Font"}), {target: {value: "Courier"}});
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(fontApi.release).not.toHaveBeenCalled(); // Undo still owns the custom font.
});
it("keeps shared bytes while another open PDF owns the same embedded font", async () => {
  await openAndChoose();
  vi.mocked(nativeAdapter.openPdf).mockResolvedValue({id:"other",name:"Other.pdf",pages:[{width:500,height:600,overlays:[{type:"text",id:"imported",x:30,y:40,text:"Other",fontSize:12,color:"#123456",fontId:id}]}]});
  fireEvent.click(screen.getByRole("button", {name:"Open PDF in new tab"}));
  await screen.findByRole("tab", {name:"Other.pdf"});
  fireEvent.click(screen.getByRole("button", {name:"Close Other.pdf"}));
  await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("other"));
  expect(fontApi.release).not.toHaveBeenCalled();
  expect(fontApi.bytes).toHaveBeenCalledOnce();
});
