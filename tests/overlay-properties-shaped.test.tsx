import { useState } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { OverlayProperties } from "../src/components/OverlayProperties";
import { shapedTextApi } from "../src/editor/shaped-text";
import type { Overlay, TextOverlay } from "../src/editor/types";

const note: TextOverlay = { type: "text", id: "note", text: "start", x: 20, y: 30, fontSize: 20, color: "#123456" };
afterEach(cleanup);
const defaults = { onDelete: vi.fn(), autoEdit: false, onAutoEdited: vi.fn(), pageWidth: 500, pageHeight: 600 };
function Editor({ initial = note, native = false }: { initial?: TextOverlay; native?: boolean }) {
  const [overlay, setOverlay] = useState<Overlay>(initial);
  const [commits, setCommits] = useState(0);
  return <><OverlayProperties {...defaults} overlay={overlay} onChooseFont={native ? () => {} : undefined} onChange={update => { setOverlay(update); setCommits(n => n + 1); }}/><output data-testid="commits">{commits}</output><output data-testid="saved">{JSON.stringify(overlay)}</output></>;
}
it("keeps composition local, then commits its complete value once", () => {
  render(<Editor/>);
  const input = screen.getByRole("textbox", { name: "Content" });
  fireEvent.compositionStart(input);
  fireEvent.change(input, { target: { value: "startに" } });
  expect(input).toHaveValue("startに");
  expect(screen.getByTestId("commits")).toHaveTextContent("0");
  fireEvent.change(input, { target: { value: "start日本" } });
  fireEvent.compositionEnd(input, { data: "日本" });
  expect(screen.getByTestId("commits")).toHaveTextContent("1");
  expect(JSON.parse(screen.getByTestId("saved").textContent!).text).toBe("start日本");
  fireEvent.input(input, { target: { value: "start日本" } });
  expect(screen.getByTestId("commits")).toHaveTextContent("1");
});
it("discards a composing draft when the selected overlay or its saved text changes", () => {
  const onChange = vi.fn();
  const view = render(<OverlayProperties {...defaults} overlay={note} onChange={onChange}/>);
  const input = screen.getByRole("textbox", { name: "Content" });
  fireEvent.compositionStart(input);
  fireEvent.change(input, { target: { value: "draft" } });
  view.rerender(<OverlayProperties {...defaults} overlay={{ ...note, id: "other", text: "other text" }} onChange={onChange}/>);
  expect(input).toHaveValue("other text");
  fireEvent.compositionEnd(input, { data: "draft" });
  expect(onChange).not.toHaveBeenCalled();
  fireEvent.compositionStart(input);
  fireEvent.change(input, { target: { value: "new draft" } });
  view.rerender(<OverlayProperties {...defaults} overlay={{ ...note, id: "other", text: "external edit" }} onChange={onChange}/>);
  expect(input).toHaveValue("external edit");
});
it("does not bubble editing shortcuts during composition", () => {
  const shortcut = vi.fn();
  render(<div onKeyDown={shortcut}><Editor/></div>);
  const input = screen.getByRole("textbox", { name: "Content" });
  fireEvent.compositionStart(input);
  fireEvent.keyDown(input, { key: "z", ctrlKey: true });
  expect(shortcut).not.toHaveBeenCalled();
});
it("requires explicit opt-in on native custom-font boxes and clears shaping for standard fonts", () => {
  render(<Editor initial={{ ...note, fontId: "custom-test" }} native/>);
  const checkbox = screen.getByRole("checkbox", { name: "Shaped text" });
  expect(checkbox).not.toBeChecked();
  expect(screen.queryByRole("combobox", { name: "Direction" })).toBeNull();
  fireEvent.click(checkbox);
  expect(JSON.parse(screen.getByTestId("saved").textContent!).shaping).toEqual({ version: 1, direction: "auto", ligatures: true });
  fireEvent.change(screen.getByRole("combobox", { name: "Direction" }), { target: { value: "rtl" } });
  fireEvent.click(screen.getByRole("checkbox", { name: "Ligatures" }));
  expect(JSON.parse(screen.getByTestId("saved").textContent!).shaping).toEqual({ version: 1, direction: "rtl", ligatures: false });
  fireEvent.change(screen.getByRole("combobox", { name: "Font" }), { target: { value: "Helvetica" } });
  expect(JSON.parse(screen.getByTestId("saved").textContent!).shaping).toBeUndefined();
});
it.each([{ native: false, initial: { ...note, fontId: "custom-test" } }, { native: false, initial: note }])("does not offer shaping without native support (%j)", props => {
  render(<Editor {...props}/>);
  expect(screen.queryByRole("checkbox", { name: "Shaped text" })).toBeNull();
});

it("keeps an active composition when native validation of the saved text fails", async () => {
  let reject!: (error: Error) => void;
  const prepare = vi.spyOn(shapedTextApi, "prepare").mockImplementation(() => new Promise((_, fail) => { reject = fail; }));
  render(<Editor initial={{ ...note, fontId: "validation-failure", text: "bad original", shaping: { version: 1, direction: "auto", ligatures: true } }} native/>);
  await waitFor(() => expect(prepare).toHaveBeenCalled());
  const input = screen.getByRole("textbox", { name: "Content" });
  fireEvent.compositionStart(input);
  fireEvent.change(input, { target: { value: "repair 日本" } });
  await act(async () => reject(new Error("Native shaping rejected this text")));
  expect(screen.getByRole("alert")).toHaveTextContent("Native shaping rejected");
  expect(input).toHaveValue("repair 日本");
  expect(screen.getByTestId("commits")).toHaveTextContent("0");
  prepare.mockRejectedValue(new Error("Unsupported repair"));
  fireEvent.compositionEnd(input);
  await screen.findByText("Unsupported repair");
  expect(input).toHaveValue("repair 日本");
  expect(JSON.parse(screen.getByTestId("saved").textContent!).text).toBe("repair 日本");
  prepare.mockRestore();
});

it("lets native text opt into shaping before choosing its first custom font", async () => {
  render(<Editor native/>);
  const checkbox = screen.getByRole("checkbox", { name: "Shaped text" });
  expect(checkbox).not.toBeChecked();
  fireEvent.click(checkbox);
  expect(checkbox).toBeChecked();
  expect(await screen.findByRole("alert")).toHaveTextContent(/choose a custom font/i);
  fireEvent.change(screen.getByRole("textbox", { name: "Content" }), { target: { value: "العربية" } });
  const saved = JSON.parse(screen.getByTestId("saved").textContent!);
  expect(saved.text).toBe("العربية");
  expect(saved.shaping).toEqual({ version: 1, direction: "auto", ligatures: true });
  expect(screen.getByRole("button", { name: "More fonts…" })).toBeEnabled();
});

it("discards composition and prevents draft edits while the document is busy", () => {
  const onChange = vi.fn();
  const view = render(<OverlayProperties {...defaults} overlay={note} onChange={onChange}/>);
  const input = screen.getByRole("textbox", { name: "Content" });
  fireEvent.compositionStart(input);
  fireEvent.change(input, { target: { value: "pending composition" } });
  view.rerender(<OverlayProperties {...defaults} overlay={note} onChange={onChange} disabled/>);
  expect(input).toBeDisabled();
  expect(input).toHaveValue("start");
  fireEvent.change(input, { target: { value: "unsaved typing" } });
  fireEvent.compositionEnd(input);
  expect(onChange).not.toHaveBeenCalled();
  view.rerender(<OverlayProperties {...defaults} overlay={note} onChange={onChange}/>);
  expect(input).toHaveValue("start");
  expect(input).toBeEnabled();
});
