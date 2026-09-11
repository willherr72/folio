import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { DocumentViewport } from "../src/components/DocumentViewport";
import { PageView, Thumbnail, clearRenderCache } from "../src/components/PageView";
import { createDemoAdapter } from "../src/editor/adapter";
import { mergeHighlightCharacters } from "../src/editor/annotation-selection";
import type { PagePlan, Rotation } from "../src/editor/types";
const characters = [
  { text: "A", x: 20, y: 30, width: 10, height: 15 },
  { text: " ", x: 30, y: 30, width: 0, height: 0 },
  { text: "B", x: 35, y: 30, width: 10, height: 15 },
  { text: "\n", x: 45, y: 30, width: 0, height: 0 },
  { text: "C", x: 20, y: 50, width: 10, height: 15 },
];
const base: PagePlan = { id: "annotation-page", sourceId: "annotation-source", pageIndex: 0, width: 200, height: 300, rotation: 0, overlays: [] };
function props(overrides: Partial<React.ComponentProps<typeof PageView>> = {}) {
 return { adapter: { ...createDemoAdapter(), getPageText: async () => ({ characters }) }, page: base, pageNumber: 1, zoom: 100, tool: "highlight" as const, selectedOverlayId: null, pendingSignature: null, onHighlight: vi.fn(), onAddComment: vi.fn(), onSelectOverlay: vi.fn(), onAddText: vi.fn(), onPlaceSignature: vi.fn(), onMoveOverlay: vi.fn(), ...overrides };
}
beforeEach(() => {
 vi.stubGlobal("PointerEvent", class extends MouseEvent { pointerId = 1; });
 Object.defineProperty(SVGElement.prototype, "setPointerCapture", { configurable: true, value: vi.fn() });
 Object.defineProperty(SVGElement.prototype, "releasePointerCapture", { configurable: true, value: vi.fn() });
});
afterEach(() => { cleanup(); clearRenderCache([base.sourceId]); window.getSelection()?.removeAllRanges(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });
it.each([0, 180] as Rotation[])("merges source glyphs per horizontal line at intrinsic %i", rotation => {
 expect(mergeHighlightCharacters(characters, rotation)).toEqual([{ x: 20, y: 30, width: 25, height: 15 }, { x: 20, y: 50, width: 10, height: 15 }]);
});
it.each([90, 270] as Rotation[])("merges source glyphs per vertical line at intrinsic %i", rotation => {
 expect(mergeHighlightCharacters(characters.map(c => ({ ...c, x: c.y, y: c.x, width: c.height, height: c.width })), rotation)).toEqual([{ x: 30, y: 20, width: 15, height: 25 }, { x: 50, y: 20, width: 15, height: 10 }]);
});
it("commits source geometry only after a text-selection gesture, and ignores collapsed or unrelated gestures", async () => {
 const options = props({ zoom: 200, page: { ...base, rotation: 90 } });
 const view = render(<PageView {...options} />);
 const layer = view.getByLabelText("Page 1 text");
 await waitFor(() => expect(layer.textContent).toBe("A B\nC"));
 const range = document.createRange(); range.selectNodeContents(layer);
 window.getSelection()!.addRange(range);
 fireEvent.pointerUp(document);
 expect(options.onHighlight).not.toHaveBeenCalled();
 fireEvent.pointerDown(layer, { button: 0 }); fireEvent.pointerUp(document);
 expect(options.onHighlight).toHaveBeenCalledTimes(1);
 expect(options.onHighlight).toHaveBeenCalledWith([{ x: 20, y: 30, width: 25, height: 15 }, { x: 20, y: 50, width: 10, height: 15 }]);
 await Promise.resolve();
 fireEvent.pointerDown(layer, { button: 0 }); fireEvent.pointerUp(document);
 expect(options.onHighlight).toHaveBeenCalledTimes(1);
});
it("commits each selected page before clearing a cross-page selection", async () => {
 const first = props(); const second = props({ page: { ...base, id: "second" }, pageNumber: 2 });
 const view = render(<div className="document-viewport"><PageView {...first} /><PageView {...second} /></div>);
 await waitFor(() => expect(view.getByLabelText("Page 2 text").textContent).toBe("A B\nC"));
 const start = view.getByLabelText("Page 1 text"); const end = view.getByLabelText("Page 2 text");
 fireEvent.pointerDown(start, { button: 0 });
 const range = document.createRange(); range.setStart(start.firstChild!.firstChild!, 0); range.setEnd(end.lastChild!.firstChild!, 1);
 window.getSelection()!.addRange(range); fireEvent.pointerUp(end);
 expect(first.onHighlight).toHaveBeenCalledTimes(1); expect(second.onHighlight).toHaveBeenCalledTimes(1);
});
it("renders highlights behind selectable text, selects without dragging, and renders comments in thumbnails", async () => {
 const page: PagePlan = { ...base, overlays: [{ type: "highlight", id: "mark", rects: [{ x: 20, y: 30, width: 25, height: 15 }], color: "#ffff00", opacity: .25 }, { type: "comment", id: "note", x: 60, y: 80, text: "Review this", color: "#ffff00" }] };
 const options = props({ page, tool: "select" }); const view = render(<PageView {...options} />);
 const svg = view.container.querySelector("svg")!;
 vi.spyOn(svg, "getBoundingClientRect").mockReturnValue({ left: 0, top: 0, width: 200, height: 300 } as DOMRect);
 const mark = view.container.querySelector("[data-highlight-id=mark]")!;
 expect(mark.querySelector("rect")).toHaveStyle("fill-opacity: 0.25");
 expect(mark.compareDocumentPosition(view.getByLabelText("Page 1 text")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
 fireEvent.pointerDown(view.getByLabelText("Page 1 text"), { clientX: 25, clientY: 35, button: 0 });
 expect(options.onSelectOverlay).toHaveBeenLastCalledWith("mark"); expect(options.onMoveOverlay).not.toHaveBeenCalled();
 fireEvent.pointerDown(view.container.querySelector("[data-overlay=note]")!, { clientX: 65, clientY: 85, button: 0 });
 expect(options.onMoveOverlay).toHaveBeenLastCalledWith("note", 60, 80, "start");
 const thumbnail = render(<Thumbnail adapter={options.adapter} page={page} />);
 expect(thumbnail.container.querySelector(".annotation-highlight rect")).toHaveStyle("fill-opacity: 0.25"); expect(thumbnail.container.querySelector(".annotation-comment")).not.toBeNull();
});
it("places comments in source coordinates and explains pages without embedded text", async () => {
 const options = props({ tool: "comment", zoom: 200, page: { ...base, rotation: 90 } }); const view = render(<PageView {...options} />);
 const svg = view.container.querySelector("svg")!;
 vi.spyOn(svg, "getBoundingClientRect").mockReturnValue({ left: 10, top: 20, width: 600, height: 400 } as DOMRect);
 fireEvent.pointerDown(svg, { clientX: 510, clientY: 100, button: 0 });
 expect(options.onAddComment).toHaveBeenCalledWith({ x: 40, y: 50 });
 clearRenderCache([base.sourceId]);
 view.rerender(<PageView {...options} tool="highlight" adapter={{ ...createDemoAdapter(), getPageText: async () => ({ characters: [] }) }} />);
 expect(await view.findByText(/No embedded text.*highlight/i)).toBeVisible();
});
it("retains traversed pages and commits glyphs loaded during a long cross-page gesture", async () => {
 const pages = Array.from({ length: 8 }, (_, index) => ({ ...base, id: `long-${index}`, pageIndex: index }));
 vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
   const top = this.classList.contains("document-page") ? 38 + Number(this.dataset.pageId?.split("-")[1]) * 324 - (this.closest(".document-viewport")?.scrollTop ?? 0) : 0;
   return { x: 0, y: top, left: 0, top, right: 500, bottom: top + (this.classList.contains("document-page") ? 300 : 400), width: 500, height: this.classList.contains("document-page") ? 300 : 400, toJSON() {} };
 });
 const onHighlight = vi.fn(); const options = props();
 const view = render(<DocumentViewport {...options} pages={pages} selectedPageId={pages[0].id} onSelectPage={vi.fn()} onZoomChange={vi.fn()} viewMode="continuous" penColor="#ffff00" penWidth={2} interactionDisabled={false} navigationRequest={null} onDraw={vi.fn()} onHighlight={onHighlight} onAddText={vi.fn()} onAddComment={vi.fn()} onPlaceSignature={vi.fn()} onMoveOverlay={vi.fn()} onSelectOverlay={vi.fn()} onEditText={undefined} />);
 await waitFor(() => expect(view.getByLabelText("Page 1 text").textContent).toBe("A B\nC"));
 const start = view.getByLabelText("Page 1 text"); fireEvent.pointerDown(start, { button: 0 });
 const host = view.getByRole("main", { name: "Document" }); host.scrollTop = 1658; fireEvent.scroll(host);
 expect(view.queryByLabelText("Page 2 text")).not.toBeNull();
 await waitFor(() => expect(view.getByLabelText("Page 6 text").textContent).toBe("A B\nC"));
 const end = view.getByLabelText("Page 6 text"); const range = document.createRange(); range.setStart(start.firstChild!.firstChild!, 0); range.setEnd(end.lastChild!.firstChild!, 1); window.getSelection()!.addRange(range);
 fireEvent.pointerUp(end);
 expect(onHighlight.mock.calls.map(call => call[0])).toEqual(pages.slice(0, 6).map(page => page.id));
 await act(async () => { await new Promise(resolve => setTimeout(resolve, 0)); });
 expect(view.queryByLabelText("Page 1 text")).toBeNull();
});

it("cancels a pending highlight when the tool changes or the pointer is cancelled", async () => {
 const options = props(); const view = render(<PageView {...options} />);
 const layer = view.getByLabelText("Page 1 text"); await waitFor(() => expect(layer.textContent).toBe("A B\nC"));
 fireEvent.pointerDown(layer, { button: 0 });
 const range = document.createRange(); range.selectNodeContents(layer); window.getSelection()!.addRange(range);
 view.rerender(<PageView {...options} tool="select" />); fireEvent.pointerUp(document);
 expect(options.onHighlight).not.toHaveBeenCalled();
 view.rerender(<PageView {...options} />); fireEvent.pointerDown(layer, { button: 0 }); fireEvent.pointerCancel(document); fireEvent.pointerUp(document);
 expect(options.onHighlight).not.toHaveBeenCalled();
});
