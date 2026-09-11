import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { clearRenderCache, Thumbnail } from "../src/components/PageView";
import type { FolioAdapter } from "../src/editor/adapter";
import type { PagePlan } from "../src/editor/types";

const sourceId = "thumbnail-viewport";
const observers = new Map<Element, IntersectionObserverCallback>();
const liveUrls = new Set<string>();
let renderedCount = 0;
function page(pageIndex: number): PagePlan {
  return { id: `thumbnail-${pageIndex}`, sourceId, pageIndex, width: 612, height: 792, rotation: 0, overlays: [] };
}
function adapter(renderPage: FolioAdapter["renderPage"] = async (_source, index) => {
  const url = `blob:thumbnail-${index}-${++renderedCount}`;
  liveUrls.add(url);
  return url;
}): FolioAdapter {
  return { kind: "native", renderPage, openPdf: async () => null, exportPdf: async () => null, closeDocument: async () => {}, engineStatus: async () => "test" };
}
async function intersect(element: Element, isIntersecting: boolean) {
  await act(async () => {
    observers.get(element)?.([{ target: element, isIntersecting } as IntersectionObserverEntry], {} as IntersectionObserver);
  });
}
beforeEach(() => {
  renderedCount = 0;
  liveUrls.clear();
  vi.stubGlobal("IntersectionObserver", class {
    private targets = new Set<Element>();
    constructor(private callback: IntersectionObserverCallback) {}
    observe(target: Element) { this.targets.add(target); observers.set(target, this.callback); }
    disconnect() { for (const target of this.targets) observers.delete(target); }
  });
  Object.defineProperty(URL, "revokeObjectURL", { configurable: true, value: vi.fn((url: string) => liveUrls.delete(url)) });
});
afterEach(async () => {
  cleanup();
  clearRenderCache([sourceId]);
  await act(async () => {});
  observers.clear();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

it("unmounts a thumbnail outside the viewport and reuses its cached raster on return", async () => {
  const view = render(<Thumbnail adapter={adapter()} page={page(0)} />);
  const host = view.container.querySelector(".thumbnail-render")!;
  expect(view.container.querySelector("image")).toBeNull();
  await intersect(host, true);
  const url = view.container.querySelector("image")!.getAttribute("href");
  await intersect(host, false);
  expect(view.container.querySelector(".thumbnail-canvas")).toBeNull();
  expect(liveUrls.has(url!)).toBe(true);
  await intersect(host, true);
  expect(view.container.querySelector("image")).toHaveAttribute("href", url);
  expect(renderedCount).toBe(1);
});

it("bounds retained rasters after visiting 300 thumbnails and renders an evicted revisit", async () => {
  const engine = adapter();
  const view = render(<>{Array.from({ length: 300 }, (_, index) => <Thumbnail key={index} adapter={engine} page={page(index)} />)}</>);
  const hosts = view.container.querySelectorAll(".thumbnail-render");
  for (const host of hosts) { await intersect(host, true); await intersect(host, false); }
  expect(renderedCount).toBe(300);
  expect(liveUrls.size).toBeLessThanOrEqual(60);
  expect(view.container.querySelectorAll(".thumbnail-canvas")).toHaveLength(0);
  expect(liveUrls.has("blob:thumbnail-0-1")).toBe(false);
  await intersect(hosts[0], true);
  expect(view.container.querySelector("image")).toHaveAttribute("href", "blob:thumbnail-0-301");
  expect(liveUrls.has("blob:thumbnail-0-301")).toBe(true);
  expect(liveUrls.size).toBeLessThanOrEqual(60);
});

it("uses the latest visibility when entering and leaving are delivered together", async () => {
  const view = render(<Thumbnail adapter={adapter()} page={page(0)} />);
  const host = view.container.querySelector(".thumbnail-render")!;
  await intersect(host, true);
  await act(async () => {
    observers.get(host)?.([
      { target: host, isIntersecting: true },
      { target: host, isIntersecting: false },
    ] as IntersectionObserverEntry[], {} as IntersectionObserver);
  });
  expect(view.container.querySelector(".thumbnail-canvas")).toBeNull();
});

it("releases a late thumbnail render after leaving view and closing its source", async () => {
  let finish!: (url: string) => void;
  const view = render(<Thumbnail adapter={adapter(() => new Promise(resolve => { finish = resolve; }))} page={page(0)} />);
  const host = view.container.querySelector(".thumbnail-render")!;
  await intersect(host, true);
  await intersect(host, false);
  clearRenderCache([sourceId]);
  view.unmount();
  await act(async () => { finish("blob:thumbnail-late"); });
  expect(URL.revokeObjectURL).toHaveBeenCalledWith("blob:thumbnail-late");
  expect(URL.revokeObjectURL).toHaveBeenCalledTimes(1);
});
