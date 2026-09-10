import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { clearRenderCache, Thumbnail } from "../src/components/PageView";
import type { FolioAdapter } from "../src/editor/adapter";
import type { PagePlan } from "../src/editor/types";

const pages: PagePlan[] = Array.from({ length: 65 }, (_, pageIndex) => ({
  id: `page-${pageIndex}`,
  sourceId: "stress-source",
  pageIndex,
  width: 612,
  height: 792,
  rotation: 0,
  overlays: [],
}));

const adapter: FolioAdapter = {
  kind: "native",
  async openPdf() { return null; },
  async renderPage(_sourceId, pageIndex) { return `blob:page-${pageIndex}`; },
  async exportPdf() { return null; },
  async closeDocument() {},
  async engineStatus() { return "test"; },
};

afterEach(() => {
  cleanup();
  clearRenderCache(["stress-source"]);
  vi.restoreAllMocks();
});

describe("render cache ownership", () => {
  it("does not revoke a mounted preview when more than 60 pages render", async () => {
    const revoke = vi.fn();
    Object.defineProperty(URL, "revokeObjectURL", { configurable: true, value: revoke });
    const view = render(<div>{pages.map((page) => <Thumbnail key={page.id} adapter={adapter} page={page} />)}</div>);

    await waitFor(() => expect(view.container.querySelectorAll("image")).toHaveLength(65));

    expect(view.container.querySelector('image[href="blob:page-0"]')).toBeInTheDocument();
    expect(revoke).not.toHaveBeenCalledWith("blob:page-0");
  });

  it("preserves repeated spaces in thumbnail text", async () => {
    Object.defineProperty(URL, "revokeObjectURL", { configurable: true, value: vi.fn() });
    const page: PagePlan = {
      ...pages[0],
      overlays: [{ type: "text", id: "spaced-text", x: 40, y: 50, text: "A   B", fontSize: 18, color: "#2D2A26" }],
    };
    const view = render(<Thumbnail adapter={adapter} page={page} />);

    await waitFor(() => expect(view.container.querySelector("text")).toBeInTheDocument());
    const text = view.container.querySelector("text")!;
    expect(text.textContent).toBe("A   B");
    expect(text.getAttribute("xml:space")).toBe("preserve");
    expect((text as SVGTextElement).style.whiteSpace).toBe("pre");
  });
});