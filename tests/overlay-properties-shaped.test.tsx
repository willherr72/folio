import { useState } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { OverlayProperties } from "../src/components/OverlayProperties";
import { shapedTextApi } from "../src/editor/shaped-text";
import type { Overlay, TextOverlay } from "../src/editor/types";

const note: TextOverlay = { type: "text", id: "note", text: "start", x: 20, y: 30, fontSize: 20, color: "#123456" };
afterEach(cleanup);
const defaults = { onDelete: vi.fn(), autoEdit: false, onAutoEdited: vi.fn(), pageWidth: 500, pageHeight: 600 };
it("edits area dimensions and layout while retaining source text and font size", () => {
 render(<Editor initial={{...note,text:"original\r\ntext",fontId:"area-properties",shaping:{version:1,direction:"ltr",ligatures:true},area:{version:1,width:180,height:240,inset:4,lineSpacing:1.2,alignment:"left"}}}/>);
 fireEvent.change(screen.getByRole("spinbutton",{name:"Area width (pt)"}),{target:{value:"220"}});
 fireEvent.blur(screen.getByRole("spinbutton",{name:"Area width (pt)"}));
 fireEvent.change(screen.getByRole("spinbutton",{name:"Area height (pt)"}),{target:{value:"180"}});
 fireEvent.blur(screen.getByRole("spinbutton",{name:"Area height (pt)"}));
 fireEvent.change(screen.getByRole("combobox",{name:"Alignment"}),{target:{value:"center"}});
 fireEvent.change(screen.getByRole("spinbutton",{name:"Line spacing"}),{target:{value:"1.5"}});
 fireEvent.blur(screen.getByRole("spinbutton",{name:"Line spacing"}));
 fireEvent.change(screen.getByRole("spinbutton",{name:"Inset (pt)"}),{target:{value:"8"}});
 fireEvent.blur(screen.getByRole("spinbutton",{name:"Inset (pt)"}));
 expect(JSON.parse(screen.getByTestId("saved").textContent!)).toMatchObject({text:"original\r\ntext",fontSize:20,area:{width:220,height:180,inset:8,lineSpacing:1.5,alignment:"center"}});
 const commits=screen.getByTestId("commits").textContent;
 fireEvent.change(screen.getByRole("spinbutton",{name:"Area width (pt)"}),{target:{value:""}});
 expect(screen.getByTestId("commits")).toHaveTextContent(commits!);
});
it("allows unfinished area numbers without committing and restores cancelled or empty drafts", () => {
 render(<Editor initial={{...note,area:{version:1,width:180,height:240,inset:4,lineSpacing:1.2,alignment:"left"}}}/>);
 const width=screen.getByRole("spinbutton",{name:"Area width (pt)"});
 for(const value of ["","2","24","240"]) fireEvent.change(width,{target:{value}});
 expect(width).toHaveValue(240);expect(screen.getByTestId("commits")).toHaveTextContent("0");
 fireEvent.blur(width);expect(screen.getByTestId("commits")).toHaveTextContent("1");
 expect(JSON.parse(screen.getByTestId("saved").textContent!).area.width).toBe(240);
 fireEvent.change(width,{target:{value:"300"}});fireEvent.keyDown(width,{key:"Escape"});fireEvent.blur(width);
 expect(width).toHaveValue(240);expect(screen.getByTestId("commits")).toHaveTextContent("1");
 fireEvent.change(width,{target:{value:""}});fireEvent.blur(width);expect(width).toHaveValue(240);
 const spacing=screen.getByRole("spinbutton",{name:"Line spacing"});
 for(const value of ["","1","1.5"]) fireEvent.change(spacing,{target:{value}});
 expect(screen.getByTestId("commits")).toHaveTextContent("1");fireEvent.blur(spacing);
 expect(JSON.parse(screen.getByTestId("saved").textContent!).area.lineSpacing).toBe(1.5);
});
it.each([false,true])("preserves untouched mixed line endings when editing an area (composition=%s)", composing => {
 const original="alpha\r\n\r\nbravo\ncharlie";
 render(<Editor initial={{...note,text:original,fontId:"area-edit",shaping:{version:1,direction:"ltr",ligatures:true},area:{version:1,width:180,height:240,inset:5,lineSpacing:1.2,alignment:"left"}}}/>);
 const input=screen.getByRole("textbox",{name:"Content"});
 if(composing) fireEvent.compositionStart(input);
 fireEvent.change(input,{target:{value:"!alpha\n\nbravo\ncharlie"}});
 if(composing) fireEvent.compositionEnd(input);
 expect(JSON.parse(screen.getByTestId("saved").textContent!).text).toBe("!"+original);
 fireEvent.change(input,{target:{value:"!alpha\n\nbrvo\ncharlie"}});
 expect(JSON.parse(screen.getByTestId("saved").textContent!).text).toBe("!alpha\r\n\r\nbrvo\ncharlie");
});
it("keeps restored areas in custom-font LTR shaping while allowing text edits", () => {
 const area={version:1 as const,width:180,height:240,inset:5,lineSpacing:1.2,alignment:"left" as const};
 render(<Editor native initial={{...note,fontId:"area-font",shaping:{version:1,direction:"ltr",ligatures:true},area}}/>);
 expect(screen.getByRole("combobox",{name:"Font"})).toBeDisabled();
 expect(screen.getByRole("combobox",{name:"Direction"})).toBeDisabled();
 expect(screen.getByRole("checkbox",{name:"Shaped text"})).toBeDisabled();
 fireEvent.change(screen.getByRole("textbox",{name:"Content"}),{target:{value:"new\nparagraph"}});
 const saved=JSON.parse(screen.getByTestId("saved").textContent!);
 expect(saved.area).toEqual(area);
 expect(saved.text).toBe("new\nparagraph");
 expect(saved.shaping.direction).toBe("ltr");
});
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

const shaping = { version: 1, direction: "auto", ligatures: true } as const;
it.each([
  ["a\u0301\u0323b", "Backspace", 3, 3, 0, 3, "b"],
  ["x😀a\u0301y", "Delete", 2, 4, 1, 5, "xy"],
  ["office", "Backspace", 3, 3, 2, 3, "ofice"],
  ["אב\u05b0 ג", "Backspace", 3, 3, 1, 3, "א ג"],
] as const)("prepares one native %s edit with a whole-grapheme selection", (text, key, start, end, expectedStart, expectedEnd, saved) => {
  render(<Editor initial={{ ...note, text, shaping }}/>);
  const input = screen.getByRole("textbox", { name: "Content" }) as HTMLTextAreaElement;
  input.setSelectionRange(start, end);
  expect(fireEvent.keyDown(input, { key })).toBe(true);
  expect([input.selectionStart, input.selectionEnd]).toEqual([expectedStart, expectedEnd]);
  expect(input.value).toBe(text);
  expect(screen.getByTestId("commits")).toHaveTextContent("0");
  // jsdom has no native editing: supply its resulting input once, then a duplicate event.
  fireEvent.input(input, { target: { value: saved, selectionStart: expectedStart, selectionEnd: expectedStart } });
  fireEvent.change(input, { target: { value: saved } });
  expect(JSON.parse(screen.getByTestId("saved").textContent!).text).toBe(saved);
  expect(screen.getByTestId("commits")).toHaveTextContent("1");
  expect([input.selectionStart, input.selectionEnd]).toEqual([expectedStart, expectedStart]);
});

it("prepares a non-keyboard beforeinput deletion without canceling the native edit", () => {
  render(<Editor initial={{ ...note, text: "a\u0301b", shaping }}/>);
  const input = screen.getByRole("textbox", { name: "Content" }) as HTMLTextAreaElement;
  input.setSelectionRange(2, 2);
  expect(fireEvent(input, new InputEvent("beforeinput", { bubbles: true, cancelable: true, inputType: "deleteContentBackward" }))).toBe(true);
  expect([input.selectionStart, input.selectionEnd]).toEqual([0, 2]);
  expect(screen.getByTestId("commits")).toHaveTextContent("0");
});

it.each([{ ctrlKey: true }, { altKey: true }, { metaKey: true }, { shiftKey: true }, { isComposing: true }, { keyCode: 229 }])("leaves native special deletion alone: %j", keys => {
  render(<Editor initial={{ ...note, text: "a\u0301b", shaping }}/>);
  const input = screen.getByRole("textbox", { name: "Content" }) as HTMLTextAreaElement;
  input.setSelectionRange(2, 2);
  expect(fireEvent.keyDown(input, { key: "Backspace", ...keys })).toBe(true);
  fireEvent(input, new InputEvent("beforeinput", { bubbles: true, cancelable: true, inputType: "deleteContentBackward" }));
  expect([input.selectionStart, input.selectionEnd]).toEqual([2, 2]);
});
it("leaves legacy deletion and active composition selections alone", () => {
  const view = render(<Editor initial={{ ...note, text: "a\u0301b" }}/>);
  const input = screen.getByRole("textbox", { name: "Content" }) as HTMLTextAreaElement;
  input.setSelectionRange(2, 2);
  fireEvent.keyDown(input, { key: "Backspace" });
  fireEvent(input, new InputEvent("beforeinput", { bubbles: true, cancelable: true, inputType: "deleteContentBackward" }));
  expect([input.selectionStart, input.selectionEnd]).toEqual([2, 2]);
  view.unmount();
  render(<Editor initial={{ ...note, text: "a\u0301b", shaping }}/>);
  const composingInput = screen.getByRole("textbox", { name: "Content" }) as HTMLTextAreaElement;
  fireEvent.compositionStart(composingInput);
  composingInput.setSelectionRange(2, 2);
  fireEvent.keyDown(composingInput, { key: "Backspace" });
  fireEvent(composingInput, new InputEvent("beforeinput", { bubbles: true, cancelable: true, inputType: "deleteContentBackward" }));
  expect([composingInput.selectionStart, composingInput.selectionEnd]).toEqual([2, 2]);
});

it("does not prepare deletion while disabled, and removes beforeinput handling on legacy text", () => {
  const onChange = vi.fn();
  const overlay = { ...note, text: "a\u0301b", shaping };
  const view = render(<OverlayProperties {...defaults} overlay={overlay} onChange={onChange} disabled/>);
  const input = screen.getByRole("textbox", { name: "Content" }) as HTMLTextAreaElement;
  input.setSelectionRange(2, 2);
  fireEvent.keyDown(input, { key: "Backspace" });
  fireEvent(input, new InputEvent("beforeinput", { bubbles: true, cancelable: true, inputType: "deleteContentBackward" }));
  expect([input.selectionStart, input.selectionEnd]).toEqual([2, 2]);
  view.rerender(<OverlayProperties {...defaults} overlay={overlay} onChange={onChange}/>);
  fireEvent.keyDown(input, { key: "Backspace" });
  expect([input.selectionStart, input.selectionEnd]).toEqual([0, 2]);
  view.rerender(<OverlayProperties {...defaults} overlay={{ ...overlay, shaping: undefined }} onChange={onChange}/>);
  input.setSelectionRange(2, 2);
  fireEvent(input, new InputEvent("beforeinput", { bubbles: true, cancelable: true, inputType: "deleteContentBackward" }));
  expect([input.selectionStart, input.selectionEnd]).toEqual([2, 2]);
  expect(onChange).not.toHaveBeenCalled();
});
it("re-enables non-keyboard deletion after a modifier key is released", () => {
  render(<Editor initial={{ ...note, text: "a\u0301b", shaping }}/>);
  const input = screen.getByRole("textbox", { name: "Content" }) as HTMLTextAreaElement;
  input.setSelectionRange(2, 2);
  fireEvent.keyDown(input, { key: "Control", ctrlKey: true });
  fireEvent.keyUp(input, { key: "Control" });
  fireEvent(input, new InputEvent("beforeinput", { bubbles: true, cancelable: true, inputType: "deleteContentBackward" }));
  expect([input.selectionStart, input.selectionEnd]).toEqual([0, 2]);
});
it.each([
  { inputType: "insertText", cancelable: true },
  { inputType: "historyUndo", cancelable: true },
  { inputType: "deleteContentBackward", cancelable: false },
  { inputType: "deleteContentBackward", cancelable: true, isComposing: true },
])("does not alter unrelated or composing native input: %j", event => {
  render(<Editor initial={{ ...note, text: "a\u0301b", shaping }}/>);
  const input = screen.getByRole("textbox", { name: "Content" }) as HTMLTextAreaElement;
  input.setSelectionRange(2, 2);
  fireEvent(input, new InputEvent("beforeinput", { bubbles: true, ...event }));
  expect([input.selectionStart, input.selectionEnd]).toEqual([2, 2]);
});
