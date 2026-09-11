import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { PdfTextLayer, clearPageTextCache } from "../src/components/PdfTextLayer";
import { createDemoAdapter } from "../src/editor/adapter";
import { selectedHighlightRects } from "../src/editor/annotation-selection";
import { findPageMatches } from "../src/editor/search";
import type { PagePlan, PageText } from "../src/editor/types";

const plan: PagePlan = { id: "selection-page", sourceId: "selection-source", pageIndex: 0, width: 300, height: 160, rotation: 0, overlays: [] };
const shared: PageText = { characters: [
  { text: "f", x: 18, y: 20, width: 12, height: 24 },
  { text: "f", x: 18.005, y: 20, width: 12, height: 24 },
  { text: "i", x: 30, y: 20, width: 5, height: 24 },
] };
async function surface(text = shared) {
  const adapter = { ...createDemoAdapter(), getPageText: async () => text };
  const view = render(<div className="document-viewport"><svg><PdfTextLayer adapter={adapter} page={plan} pageNumber={1} selectable /></svg></div>);
  const layer = view.getByLabelText("Page 1 text");
  await waitFor(() => expect(layer.textContent).toBe(text.characters.map(c => c.text).join("")));
  return { view, layer };
}
function select(node: Node, start: number, end: number) {
  const range = document.createRange(); range.setStart(node, start); range.setEnd(node, end);
  const selection = window.getSelection()!; selection.removeAllRanges(); selection.addRange(range);
  return selection;
}
afterEach(() => { cleanup(); clearPageTextCache([plan.sourceId, "other-document"]); window.getSelection()?.removeAllRanges(); vi.restoreAllMocks(); });

it("gives coincident ligature characters one selectable span without merging adjacent glyphs", async () => {
  const { layer } = await surface();
  const spans = layer.querySelectorAll("[data-pdf-character]");
  expect(Array.from(spans, span => span.textContent)).toEqual(["ff", "i"]);
});
it("copies a ligature interior exactly and highlights the entire shared source box", async () => {
  const { layer } = await surface();
  const node = layer.querySelector("[data-pdf-character]")!.firstChild!;
  expect(node.textContent).toBe("ff");
  const selection = select(node, 1, 2);
  const setData = vi.fn(); fireEvent.copy(document, { clipboardData: { setData } });
  expect(setData).toHaveBeenCalledWith("text/plain", "f");
  const rects = selectedHighlightRects(layer, shared.characters, selection);
  expect(rects).toHaveLength(1); expect(rects[0].x).toBe(18); expect(rects[0].width).toBeCloseTo(12.005);
  const next = layer.querySelectorAll("[data-pdf-character]")[1].firstChild!;
  expect(selectedHighlightRects(layer, shared.characters, select(next, 0, 1))).toEqual([{ x: 30, y: 20, width: 5, height: 24 }]);
});
it("keeps a supplementary character intact when a range boundary lands inside its surrogate pair", async () => {
  const { layer } = await surface({ characters: [{ text: "\u{1d434}", x: 20, y: 20, width: 18, height: 24 }] });
  const node = layer.querySelector("[data-pdf-character]")!.firstChild!;
  for (const [start, end] of [[0, 1], [1, 2]]) {
    select(node, start, end);
    const setData = vi.fn(); fireEvent.copy(document, { clipboardData: { setData } });
    expect(setData).toHaveBeenCalledWith("text/plain", "\u{1d434}");
  }
});
it("preserves combining sequences without normalizing copied text", async () => {
  const text = { characters: Array.from("q\u0307\u0323", text => ({ text, x: 20, y: 20, width: 18, height: 24 })) };
  const { layer } = await surface(text);
  expect(layer.querySelectorAll("[data-pdf-character]")).toHaveLength(1);
  select(layer.firstChild!.firstChild!, 0, 3);
  const setData = vi.fn(); fireEvent.copy(document, { clipboardData: { setData } });
  expect(setData).toHaveBeenCalledWith("text/plain", "q\u0307\u0323");
  expect(findPageMatches(plan, text, "\u0323")[0].rects).toEqual([{ x: 20, y: 20, width: 18, height: 24 }]);
});
it("keeps separate lines and materially different boxes separate", async () => {
  const text = { characters: [shared.characters[0], { ...shared.characters[0], text: "\n" }, shared.characters[0], { ...shared.characters[0], x: 18.1 }] };
  const { layer } = await surface(text);
  expect(layer.querySelectorAll("[data-pdf-character]")).toHaveLength(4);
});

it("copies across pages only inside the originating document viewport", async () => {
  const adapter = { ...createDemoAdapter(), getPageText: async (source: string) => ({ characters: [{ text: source === plan.sourceId ? "A" : "B", x: 20, y: 20, width: 12, height: 24 }] }) };
  const view = render(<>
    <div className="document-viewport"><svg><PdfTextLayer adapter={adapter} page={plan} pageNumber={1} selectable /><PdfTextLayer adapter={adapter} page={{ ...plan, id: "duplicate" }} pageNumber={2} selectable /></svg></div>
    <div className="document-viewport"><svg><PdfTextLayer adapter={adapter} page={{ ...plan, id: "other", sourceId: "other-document" }} pageNumber={3} selectable /></svg></div>
  </>);
  await waitFor(() => expect(view.getByLabelText("Page 3 text").textContent).toBe("B"));
  const nodes = view.container.querySelectorAll("[data-pdf-character]");
  const range = document.createRange(); range.setStart(nodes[0].firstChild!, 0); range.setEnd(nodes[2].firstChild!, 1);
  window.getSelection()!.addRange(range);
  const setData = vi.fn(); fireEvent.copy(document, { clipboardData: { setData } });
  expect(setData).toHaveBeenCalledWith("text/plain", "AA");
});
