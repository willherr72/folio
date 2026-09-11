import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { App } from "../src/App";
import { nativeAdapter } from "../src/editor/adapter";

vi.mock("../src/components/DocumentViewport", () => ({ DocumentViewport: (props: any) => <button onClick={() => props.onAddText(props.pages[0].id, {x: 40, y: 70})}>Place added text</button> }));
beforeEach(() => {
  localStorage.clear();
  vi.spyOn(nativeAdapter, "openPdf").mockResolvedValue({ id: "fonts", name: "Fonts.pdf", pages: [{ width: 500, height: 600 }] });
  vi.spyOn(nativeAdapter, "renderPage").mockResolvedValue("data:image/png;base64,");
  vi.spyOn(nativeAdapter, "exportPdf").mockResolvedValue("Saved.pdf");
  vi.spyOn(nativeAdapter, "closeDocument").mockResolvedValue();
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
it("saves a chosen font and preserves font changes in undo/redo", async () => {
  render(<App initialDemo={false}/>);
  fireEvent.click(screen.getByRole("button", {name: "Open a PDF"}));
  await screen.findByRole("tab", {name: "Fonts.pdf"});
  fireEvent.click(screen.getByRole("button", {name: "Text"}));
  fireEvent.click(screen.getByRole("button", {name: "Place added text"}));
  expect(await screen.findByRole("textbox", {name: "Content"})).toHaveFocus();
  fireEvent.change(screen.getByRole("textbox", {name: "Content"}), {target: {value: "A different font"}});
  fireEvent.change(screen.getByRole("combobox", {name: "Font"}), {target: {value: "Courier-BoldOblique"}});
  fireEvent.click(screen.getByRole("button", {name: "Undo"}));
  expect(screen.getByRole("combobox", {name: "Font"})).toHaveValue("Helvetica");
  fireEvent.click(screen.getByRole("button", {name: "Redo"}));
  expect(screen.getByRole("combobox", {name: "Font"})).toHaveValue("Courier-BoldOblique");
  fireEvent.click(screen.getByRole("button", {name: "Save a copy"}));
  await waitFor(() => expect(nativeAdapter.exportPdf).toHaveBeenCalledOnce());
  expect(vi.mocked(nativeAdapter.exportPdf).mock.calls[0][0][0].overlays[0]).toMatchObject({type: "text", text: "A different font", fontName: "Courier-BoldOblique", x: 40, y: 70});
});
