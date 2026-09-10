import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PageView, clearRenderCache } from "../src/components/PageView";
import { createDemoAdapter } from "../src/editor/adapter";
import type { PagePlan, Rotation } from "../src/editor/types";

const adapter = createDemoAdapter();
const base: PagePlan = { id: "p1", sourceId: "drawing", pageIndex: 0, width: 200, height: 300, rotation: 0, overlays: [] };
beforeEach(() => {
  vi.stubGlobal("PointerEvent", class extends MouseEvent { pointerId: number; constructor(type: string, init: PointerEventInit = {}) { super(type, init); this.pointerId = init.pointerId ?? 1; } });
  Object.defineProperty(SVGElement.prototype, "setPointerCapture", { configurable: true, value: vi.fn() });
  Object.defineProperty(SVGElement.prototype, "releasePointerCapture", { configurable: true, value: vi.fn() });
});
afterEach(() => { cleanup(); clearRenderCache(["drawing"]); vi.unstubAllGlobals(); vi.restoreAllMocks(); });
function setup(rotation: Rotation = 0, disabled = false, overlays = base.overlays) {
  const onDraw = vi.fn(), onActivate = vi.fn();
  const view = render(<PageView adapter={adapter} page={{ ...base, rotation, overlays }} pageNumber={1} zoom={100} tool="draw" selectedOverlayId={null} pendingSignature={null} onSelectOverlay={vi.fn()} onAddText={vi.fn()} onPlaceSignature={vi.fn()} onMoveOverlay={vi.fn()} onDraw={onDraw} onActivate={onActivate} interactionDisabled={disabled} />);
  const svg = view.container.querySelector("svg")!;
  const width = rotation % 180 ? 300 : 200, height = rotation % 180 ? 200 : 300;
  vi.spyOn(svg, "getBoundingClientRect").mockReturnValue({ left: 10, top: 20, width, height, right: width + 10, bottom: height + 20, x: 10, y: 20, toJSON() {} });
  return { ...view, svg, onDraw, onActivate };
}
describe("page drawing", () => {
  it.each([
    [0, [{ x: 20, y: 40 }, { x: 40, y: 60 }]],
    [90, [{ x: 40, y: 280 }, { x: 60, y: 260 }]],
    [180, [{ x: 180, y: 260 }, { x: 160, y: 240 }]],
    [270, [{ x: 160, y: 20 }, { x: 140, y: 40 }]],
  ] as const)("commits one stroke in original PDF coordinates at rotation %i", (rotation, path) => {
    const { svg, onDraw, onActivate } = setup(rotation);
    fireEvent.pointerDown(svg, { pointerId: 7, clientX: 30, clientY: 60, button: 0 });
    fireEvent.pointerMove(svg, { pointerId: 7, clientX: 50, clientY: 80 });
    expect(onDraw).not.toHaveBeenCalled();
    fireEvent.pointerUp(svg, { pointerId: 7, clientX: 50, clientY: 80 });
    expect(onDraw).toHaveBeenCalledOnce(); expect(onDraw).toHaveBeenCalledWith(path);
    expect(onActivate).toHaveBeenCalledOnce();
    expect(svg.setPointerCapture).toHaveBeenCalledWith(7);
    expect(svg.releasePointerCapture).toHaveBeenCalledWith(7);
  });
  it("discards a cancelled stroke and permits the next stroke", () => {
    const { svg, onDraw } = setup();
    fireEvent.pointerDown(svg, { clientX: 30, clientY: 60 });
    fireEvent.pointerMove(svg, { clientX: 50, clientY: 80 });
    fireEvent.pointerCancel(svg);
    expect(onDraw).not.toHaveBeenCalled();
    expect(svg.querySelector(".draft-ink")).toBeNull();
    fireEvent.pointerDown(svg, { clientX: 30, clientY: 60 });
    fireEvent.pointerUp(svg, { clientX: 50, clientY: 80 });
    expect(onDraw).toHaveBeenCalledOnce();
  });
  it("starts drawing over existing annotations", () => {
    const { svg, onDraw } = setup(0, false, [{ type: "text", id: "text", x: 10, y: 20, text: "Hi", fontSize: 12, color: "#000000" }]);
    fireEvent.pointerDown(svg.querySelector("text")!, { clientX: 30, clientY: 60 });
    fireEvent.pointerUp(svg, { clientX: 50, clientY: 80 });
    expect(onDraw).toHaveBeenCalledOnce();
  });
  it("blocks canvas interactions while a modal is open", () => {
    const { svg, onDraw, onActivate } = setup(0, true);
    fireEvent.pointerDown(svg, { clientX: 30, clientY: 60 });
    fireEvent.pointerUp(svg, { clientX: 50, clientY: 80 });
    expect(onDraw).not.toHaveBeenCalled();
    expect(onActivate).not.toHaveBeenCalled();
  });
});