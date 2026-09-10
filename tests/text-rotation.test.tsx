import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { PageView, Thumbnail, clearRenderCache } from "../src/components/PageView";
import { createDemoAdapter } from "../src/editor/adapter";
import { findPageMatches } from "../src/editor/search";
import { textOverlayBounds, rotateTextRect } from "../src/editor/text-overlay-geometry";
import type { PagePlan, Rotation, TextOverlay } from "../src/editor/types";
const overlay: TextOverlay = { type: "text", id: "rotated-note", x: 100, y: 100, text: "AB\nC", fontSize: 10, color: "#000000" };
const base: PagePlan = { id: "text-rotation", sourceId: "rotation-source", pageIndex: 0, width: 240, height: 320, rotation: 0, overlays: [] };
const adapter = { ...createDemoAdapter(), renderPage: async () => "data:image/svg+xml,%3Csvg/%3E", getPageText: async () => ({ characters: [] }) };
function props(page: PagePlan) { return { adapter, page, pageNumber: 1, zoom: 170, tool: "select" as const, selectedOverlayId: overlay.id, pendingSignature: null, onSelectOverlay: vi.fn(), onAddText: vi.fn(), onPlaceSignature: vi.fn(), onMoveOverlay: vi.fn() }; }
beforeEach(() => {
 vi.stubGlobal("PointerEvent", class extends MouseEvent { pointerId = 1; });
 Object.defineProperty(SVGElement.prototype, "setPointerCapture", { configurable: true, value: vi.fn() });
 Object.defineProperty(SVGElement.prototype, "releasePointerCapture", { configurable: true, value: vi.fn() });
});
afterEach(() => { cleanup(); clearRenderCache([base.sourceId]); vi.unstubAllGlobals(); vi.restoreAllMocks(); });
it.each([
 [0, { x: 102, y: 103, width: 20, height: 10 }],
 [90, { x: 87, y: 102, width: 10, height: 20 }],
 [180, { x: 78, y: 87, width: 20, height: 10 }],
 [270, { x: 103, y: 78, width: 10, height: 20 }],
] as const)("rotates local text rectangles about the source anchor at %i degrees", (rotation, expected) => {
 expect(rotateTextRect({ ...overlay, rotation }, { x: 2, y: 3, width: 20, height: 10 })).toEqual(expected);
});
it("keeps absent rotation identical to existing text geometry", () => {
 expect(textOverlayBounds(overlay)).toEqual({ x: 96, y: 97, width: 44, height: 29 });
});
it.each([0, 90, 180, 270] as Rotation[])("rotates added-text search glyphs and multiline selection bounds at %i degrees", rotation => {
 const text = { ...overlay, rotation };
 const page = { ...base, rotation: 270 as const, overlays: [text] };
 const matches = findPageMatches(page, { characters: [] }, "ab c");
 expect(matches).toHaveLength(1);
 const expected = [rotateTextRect(text, { x: 0, y: 0, width: 11.2, height: 12 }), rotateTextRect(text, { x: 0, y: 12, width: 5.6, height: 12 })];
 expect(matches[0].rects).toHaveLength(2);
 for (let index = 0; index < 2; index++) for (const key of ["x", "y", "width", "height"] as const) expect(matches[0].rects[index][key]).toBeCloseTo(expected[index][key]);
 const options = props(page); const view = render(<PageView {...options} />);
 const node = view.container.querySelector("[data-overlay] text")!;
 expect(node.getAttribute("transform")).toBe(rotation ? `rotate(${rotation} 100 100)` : null);
 const box = textOverlayBounds(text); const selection = view.container.querySelector(".selection-box")!;
 for (const key of ["x", "y", "width", "height"] as const) expect(Number(selection.getAttribute(key))).toBeCloseTo(box[key]);
 const thumbnail = render(<Thumbnail adapter={adapter} page={page} />);
 expect(thumbnail.container.querySelector("text")?.getAttribute("transform")).toBe(rotation ? `rotate(${rotation} 100 100)` : null);
 expect(thumbnail.container.querySelectorAll("tspan")).toHaveLength(2);
});
it("drags rotated text in source coordinates without altering its angle", () => {
 const text = { ...overlay, rotation: 90 as const }; const options = props({ ...base, overlays: [text] });
 const view = render(<PageView {...options} />); const svg = view.container.querySelector("svg")!;
 vi.spyOn(svg, "getBoundingClientRect").mockReturnValue({ left: 20, top: 30, width: 408, height: 544 } as DOMRect);
 const box = textOverlayBounds(text); const node = view.container.querySelector("[data-overlay] text")!;
 fireEvent.pointerDown(node, { button: 0, clientX: 20 + (box.x + 8) * 1.7, clientY: 30 + (box.y + 8) * 1.7 });
 fireEvent.pointerMove(svg, { clientX: 20 + (box.x + 28) * 1.7, clientY: 30 + (box.y + 18) * 1.7 });
 expect(options.onMoveOverlay).toHaveBeenNthCalledWith(1, overlay.id, box.x, box.y, "start");
 const moved = options.onMoveOverlay.mock.calls[1]; expect(moved[1]).toBeCloseTo(box.x + 20); expect(moved[2]).toBeCloseTo(box.y + 10);
 expect(node.getAttribute("transform")).toBe("rotate(90 100 100)");
});