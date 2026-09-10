import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DocumentViewport } from "../src/components/DocumentViewport";
import { clearRenderCache } from "../src/components/PageView";
import { createDemoAdapter } from "../src/editor/adapter";
import type { PagePlan } from "../src/editor/types";

const pages: PagePlan[] = Array.from({ length: 8 }, (_, i) => ({ id: `p${i}`, sourceId: "viewport", pageIndex: i, width: 200, height: 300, rotation: 0, overlays: [] }));
const adapter = { ...createDemoAdapter(), renderPage: async (_source: string, page: number) => `blob:viewport-${page}` };
function rect(left: number, top: number, width: number, height: number): DOMRect { return { left, top, width, height, right: left + width, bottom: top + height, x: left, y: top, toJSON() {} }; }
beforeEach(() => {
  vi.stubGlobal("PointerEvent", MouseEvent);
  Object.defineProperty(URL, "revokeObjectURL", { configurable: true, value: vi.fn() });
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    if (this.classList.contains("document-viewport")) return rect(0, 0, 500, 400);
    if (this.classList.contains("document-page")) {
      const host = this.closest(".document-viewport")!;
      const slots = [...host.querySelectorAll<HTMLElement>(".document-page")];
      let top = 38;
      for (const slot of slots) { if (slot === this) break; top += parseFloat(slot.style.height) + 24; }
      return rect(42 - host.scrollLeft, top - host.scrollTop, parseFloat(this.style.width), parseFloat(this.style.height));
    }
    return rect(0, 0, 0, 0);
  });
});
afterEach(() => { cleanup(); clearRenderCache(["viewport"]); vi.unstubAllGlobals(); vi.restoreAllMocks(); });
function props() { return { adapter, pages, selectedPageId: "p0", selectedOverlayId: null, zoom: 100, onZoomChange: vi.fn(), viewMode: "continuous" as const, tool: "text" as const, penColor: "#123456", penWidth: 2, pendingSignature: null, interactionDisabled: false, navigationRequest: null as { pageId: string; revision: number } | null, onSelectPage: vi.fn(), onSelectOverlay: vi.fn(), onAddText: vi.fn(), onPlaceSignature: vi.fn(), onMoveOverlay: vi.fn(), onDraw: vi.fn() }; }

describe("document viewport", () => {
  it("routes an edit on a visible unselected page to that page", async () => {
    const callbacks = props();
    const view = render(<DocumentViewport {...callbacks} />);
    const svg = view.container.querySelector('[data-page-id="p1"] svg')!;
    vi.spyOn(svg, "getBoundingClientRect").mockReturnValue(rect(42, 362, 200, 300));
    fireEvent.pointerDown(svg, { button: 0, clientX: 62, clientY: 382 });
    expect(callbacks.onSelectPage).toHaveBeenCalledWith("p1");
    expect(callbacks.onAddText).toHaveBeenCalledWith("p1", { x: 20, y: 20 });
  });
  it("mounts nearby pages, releases distant full-size images and scroll-selects without jumping", async () => {
    const callbacks = props();
    const view = render(<DocumentViewport {...callbacks} />);
    await waitFor(() => expect(view.container.querySelector('[data-page-id="p0"] image')).toBeInTheDocument());
    expect(view.container.querySelector('[data-page-id="p7"] svg')).toBeNull();
    const host = view.getByRole("main", { name: "Document" });
    host.scrollTop = 1658;
    fireEvent.scroll(host);
    await waitFor(() => expect(view.container.querySelector('[data-page-id="p5"] svg')).toBeInTheDocument());
    expect(view.container.querySelector('[data-page-id="p0"] svg')).toBeNull();
    await waitFor(() => expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:viewport-0"));
    expect(callbacks.onSelectPage).toHaveBeenCalledWith("p5");
    view.rerender(<DocumentViewport {...callbacks} selectedPageId="p5" />);
    expect(host.scrollTop).toBe(1658);
  });
  it("honors explicit navigation even to an unmounted page", () => {
    const callbacks = props();
    const view = render(<DocumentViewport {...callbacks} />);
    const host = view.getByRole("main", { name: "Document" });
    view.rerender(<DocumentViewport {...callbacks} selectedPageId="p7" navigationRequest={{ pageId: "p7", revision: 1 }} />);
    expect(host.scrollTop).toBeGreaterThan(2200);
    expect(view.container.querySelector('[data-page-id="p7"] svg')).toBeInTheDocument();
  });
  it("keeps the currently read page when switching modes after an older navigation request", () => {
    const callbacks = props();
    const view = render(<DocumentViewport {...callbacks} selectedPageId="p1" navigationRequest={{ pageId: "p1", revision: 1 }} />);
    const host = view.getByRole("main", { name: "Document" });
    host.scrollTop = 1658;
    fireEvent.scroll(host);
    view.rerender(<DocumentViewport {...callbacks} selectedPageId="p5" viewMode="single" navigationRequest={{ pageId: "p1", revision: 1 }} />);
    expect(host.scrollTop).toBe(0);
    expect(view.container.querySelector('[data-page-id="p5"] svg')).toBeInTheDocument();
  });
  it("bounds PDF zoom and blocks zoom changes behind a modal", () => {
    const callbacks = props();
    const view = render(<DocumentViewport {...callbacks} zoom={200} />);
    const host = view.getByRole("main", { name: "Document" });
    fireEvent.wheel(host, { ctrlKey: true, deltaY: -100 });
    expect(callbacks.onZoomChange).not.toHaveBeenCalled();
    view.rerender(<DocumentViewport {...callbacks} zoom={50} />);
    fireEvent.wheel(host, { ctrlKey: true, deltaY: 100 });
    expect(callbacks.onZoomChange).not.toHaveBeenCalled();
    view.rerender(<DocumentViewport {...callbacks} interactionDisabled />);
    const wheel = new WheelEvent("wheel", { bubbles: true, cancelable: true, ctrlKey: true, deltaY: -100 });
    act(() => { host.dispatchEvent(wheel); });
    expect(wheel.defaultPrevented).toBe(true);
    expect(callbacks.onZoomChange).not.toHaveBeenCalled();
  });
  it("renders only the selected page in single mode", () => {
    const callbacks = props();
    const view = render(<DocumentViewport {...callbacks} viewMode="single" selectedPageId="p4" />);
    expect(view.container.querySelectorAll(".document-page")).toHaveLength(1);
    expect(view.container.querySelector('[data-page-id="p4"] svg')).toBeInTheDocument();
  });
  it("prevents browser Ctrl-wheel zoom and anchors the PDF point at the cursor", () => {
    const callbacks = props();
    const view = render(<DocumentViewport {...callbacks} />);
    const host = view.getByRole("main", { name: "Document" });
    const wheel = new WheelEvent("wheel", { bubbles: true, cancelable: true, ctrlKey: true, deltaY: -100, clientX: 142, clientY: 188 });
    act(() => { host.dispatchEvent(wheel); });
    expect(wheel.defaultPrevented).toBe(true);
    const nextZoom = callbacks.onZoomChange.mock.calls[0][0];
    expect(nextZoom).toBeGreaterThan(100);
    view.rerender(<DocumentViewport {...callbacks} zoom={nextZoom} />);
    const slot = view.container.querySelector<HTMLElement>('[data-page-id="p0"]')!.getBoundingClientRect();
    expect(slot.left + 100 * nextZoom / 100).toBeCloseTo(142);
    expect(slot.top + 150 * nextZoom / 100).toBeCloseTo(188);
    const normal = new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY: 100 });
    act(() => { host.dispatchEvent(normal); });
    expect(normal.defaultPrevented).toBe(false);
    expect(callbacks.onZoomChange).toHaveBeenCalledOnce();
  });
});