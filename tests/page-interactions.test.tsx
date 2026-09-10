import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PageView, clearRenderCache } from "../src/components/PageView";
import { createDemoAdapter } from "../src/editor/adapter";
import type { PagePlan, Rotation } from "../src/editor/types";

const base: PagePlan = { id: "selection-page", sourceId: "selection-source", pageIndex: 0, width: 200, height: 300, rotation: 0, overlays: [] };
const characters = [
  { text: "A", x: 20, y: 30, width: 10, height: 15 },
  { text: " ", x: 30, y: 30, width: 0, height: 0 },
  { text: "B", x: 35, y: 30, width: 10, height: 15 },
  { text: "\r\n", x: 45, y: 30, width: 0, height: 0 },
  { text: "C", x: 20, y: 50, width: 10, height: 15 },
];
function props(overrides: Partial<React.ComponentProps<typeof PageView>> = {}) {
  return { adapter: { ...createDemoAdapter(), getPageText: async () => ({ characters }) }, page: base, pageNumber: 1, zoom: 100, tool: "select" as const, selectedOverlayId: null, pendingSignature: null, onSelectOverlay: vi.fn(), onAddText: vi.fn(), onPlaceSignature: vi.fn(), onMoveOverlay: vi.fn(), ...overrides };
}
beforeEach(() => {
  vi.stubGlobal("PointerEvent", class extends MouseEvent { pointerId = 1; });
  Object.defineProperty(SVGElement.prototype, "setPointerCapture", { configurable: true, value: vi.fn() });
  Object.defineProperty(SVGElement.prototype, "releasePointerCapture", { configurable: true, value: vi.fn() });
});
afterEach(() => { cleanup(); clearRenderCache([base.sourceId]); window.getSelection()?.removeAllRanges(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });

describe("embedded PDF selection", () => {
  it("copies reading order and zero-size whitespace through an actual browser range", async () => {
    const view = render(<PageView {...props()} />);
    const layer = await view.findByLabelText("Page 1 text");
    const range = document.createRange();
    range.selectNodeContents(layer);
    window.getSelection()!.addRange(range);
    const setData = vi.fn();
    fireEvent.copy(document, { clipboardData: { setData } });
    expect(setData).toHaveBeenCalledWith("text/plain", "A B\r\nC");
    expect(layer.textContent).toBe("A B\r\nC");
  });
  it("copies only selected text within a multi-codepoint character entry", async () => {
    const options = props({ adapter: { ...createDemoAdapter(), getPageText: async () => ({ characters: [{ ...characters[0], text: "ffi" }] }) } });
    const view = render(<PageView {...options} />);
    await waitFor(() => expect(view.getByLabelText("Page 1 text").textContent).toBe("ffi"));
    const node = view.container.querySelector("[data-pdf-character]")!.firstChild!;
    const range = document.createRange();
    range.setStart(node, 1); range.setEnd(node, 3);
    window.getSelection()!.addRange(range);
    const setData = vi.fn();
    fireEvent.copy(document, { clipboardData: { setData } });
    expect(setData).toHaveBeenCalledWith("text/plain", "fi");
  });
  it("preserves viewing when optional text extraction rejects", async () => {
    const options = props({ adapter: { ...createDemoAdapter(), getPageText: async () => { throw new Error("text unavailable"); } } });
    const view = render(<PageView {...options} />);
    await waitFor(() => expect(view.container.querySelector("image")).not.toBeNull());
    expect(view.queryByText("text unavailable")).toBeNull();
    expect(view.container.querySelector(".pdf-text-layer")?.textContent ?? "").toBe("");
  });
  it("reuses text at a new zoom but drops closed-source geometry", async () => {
    let reads = 0;
    const options = props({ adapter: { ...createDemoAdapter(), getPageText: async () => ({ characters: [{ ...characters[0], text: String(++reads) }] }) } });
    const view = render(<PageView {...options} />);
    await waitFor(() => expect(view.getByLabelText("Page 1 text").textContent).toBe("1"));
    view.rerender(<PageView {...options} zoom={200} />);
    expect(view.getByLabelText("Page 1 text").textContent).toBe("1");
    view.unmount();
    clearRenderCache([base.sourceId]);
    const next = render(<PageView {...options} />);
    await waitFor(() => expect(next.getByLabelText("Page 1 text").textContent).toBe("2"));
  });
  it("turns off selection in drawing mode and leaves overlays above the text", async () => {
    const options = props({ page: { ...base, overlays: [{ type: "text", id: "edit", x: 20, y: 30, text: "Edit", fontSize: 12, color: "#000" }] } });
    const view = render(<PageView {...options} />);
    const layer = await view.findByLabelText("Page 1 text");
    expect(layer).toHaveAttribute("data-selectable", "true");
    expect(layer.compareDocumentPosition(view.container.querySelector("[data-overlay]")!) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    view.rerender(<PageView {...options} tool="draw" />);
    expect(view.getByLabelText("Page 1 text")).toHaveAttribute("data-selectable", "false");
  });
});

describe("signature placement preview", () => {
  it.each([0, 90, 180, 270] as Rotation[])("matches clamped placement at page edges with rotation %i and 200%% zoom", (rotation) => {
    const options = props({ page: { ...base, rotation }, zoom: 200, tool: "signature", pendingSignature: [[{ x: 10, y: 20 }, { x: 110, y: 60 }]] });
    const view = render(<PageView {...options} />);
    const svg = view.container.querySelector("svg")!;
    const width = (rotation % 180 ? 300 : 200) * 2, height = (rotation % 180 ? 200 : 300) * 2;
    vi.spyOn(svg, "getBoundingClientRect").mockReturnValue({ left: 10, top: 20, width, height } as DOMRect);
    const edges = { 0: [width, height], 90: [0, height], 180: [0, 0], 270: [width, 0] };
    const [x, y] = edges[rotation];
    fireEvent.pointerMove(svg, { clientX: 10 + x, clientY: 20 + y });
    expect(svg.querySelector(".signature-preview polyline")).toHaveAttribute("points", "145,278 200,300");
    fireEvent.pointerDown(svg, { clientX: 10 + x, clientY: 20 + y, button: 0 });
    expect(options.onPlaceSignature).toHaveBeenCalledWith({ x: 200, y: 300 });
  });
  it("hides the ghost on leave, Escape, tool change and disabled interactions", () => {
    const options = props({ tool: "signature", pendingSignature: [[{ x: 0, y: 0 }, { x: 100, y: 20 }]] });
    const view = render(<PageView {...options} />);
    const svg = view.container.querySelector("svg")!;
    vi.spyOn(svg, "getBoundingClientRect").mockReturnValue({ left: 0, top: 0, width: 200, height: 300 } as DOMRect);
    const move = () => fireEvent.pointerMove(svg, { clientX: 30, clientY: 40 });
    move(); expect(svg.querySelector(".signature-preview")).not.toBeNull();
    fireEvent.pointerLeave(svg); expect(svg.querySelector(".signature-preview")).toBeNull();
    move(); fireEvent.keyDown(window, { key: "Escape" }); move();
    expect(svg.querySelector(".signature-preview")).toBeNull();
    view.rerender(<PageView {...options} pendingSignature={[...options.pendingSignature!]} />);
    move(); expect(svg.querySelector(".signature-preview")).not.toBeNull();
    view.rerender(<PageView {...options} tool="draw" />); move();
    expect(svg.querySelector(".signature-preview")).toBeNull();
    view.rerender(<PageView {...options} interactionDisabled />); move();
    expect(svg.querySelector(".signature-preview")).toBeNull();
  });
});