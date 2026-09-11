import { useState } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { OverlayProperties } from "../src/components/OverlayProperties";
import { TextOverlayPresentation } from "../src/components/TextOverlayPresentation";
import type { TextOverlay } from "../src/editor/types";
import { textOverlayBounds, textOverlayCharacters } from "../src/editor/text-overlay-geometry";

const note: TextOverlay = { type: "text", id: "note", text: "iiiiiiiiiiiiiiii", x: 20, y: 30, fontSize: 20, color: "#123456", rotation: 90 };
afterEach(cleanup);
function Editor() {
  const [overlay, setOverlay] = useState(note);
  return <><OverlayProperties overlay={overlay} onChange={update => setOverlay(value => update(value) as TextOverlay)} onDelete={vi.fn()} autoEdit={false} onAutoEdited={vi.fn()} pageWidth={500} pageHeight={600}/><svg><TextOverlayPresentation overlay={overlay}/></svg></>;
}
it("changes added text font and style without changing its contents or position", () => {
  const view = render(<Editor/>);
  const font = screen.getByRole("combobox", { name: "Font" });
  expect(font).toHaveValue("Helvetica");
  expect(screen.getAllByRole("option")).toHaveLength(12);
  fireEvent.change(font, { target: { value: "Times-BoldItalic" } });
  const text = view.container.querySelector("svg text")!;
  expect(text).toHaveAttribute("font-family", '"Times New Roman", Times, serif');
  expect(text).toHaveAttribute("font-weight", "700");
  expect(text).toHaveAttribute("font-style", "italic");
  expect(text).toHaveAttribute("transform", "rotate(90 20 30)");
  expect(text).toHaveAttribute("fill", "#123456");
  expect(screen.getByRole("textbox", { name: "Content" })).toHaveValue(note.text);
  fireEvent.change(font, { target: { value: "Courier" } });
  expect(text).toHaveAttribute("font-family", '"Courier New", Courier, monospace');
  expect(text).toHaveAttribute("font-weight", "400");
  expect(text).toHaveAttribute("font-style", "normal");
});
it("retains Helvetica for annotations saved before font selection existed", () => {
  const view = render(<svg><TextOverlayPresentation overlay={note}/></svg>);
  expect(view.container.querySelector("text")).toHaveAttribute("font-family", "Arial, Helvetica, sans-serif");
  expect(view.container.querySelector("text")).toHaveAttribute("font-weight", "400");
});
it("uses the chosen font for selection and search geometry, including rotation", () => {
  const courier = { ...note, fontName: "Courier" as const };
  const times = { ...note, fontName: "Times-Roman" as const };
  expect(textOverlayBounds(courier).height).toBeGreaterThan(textOverlayBounds(times).height);
  const glyphs = textOverlayCharacters(courier).characters;
  expect(glyphs[1].y - glyphs[0].y).toBeCloseTo(12);
  expect(glyphs[0].height).toBeCloseTo(12);
});
