import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createDemoAdapter } from "../src/editor/adapter";
import type { PagePlan, PageText } from "../src/editor/types";
import { findPageMatches, useDocumentSearch, clearSearchTextCache, readSearchPageText } from "../src/editor/search";
import { DocumentViewport } from "../src/components/DocumentViewport";
import { SearchBar } from "../src/components/SearchBar";

const page: PagePlan = { id: "page-a", sourceId: "source-search", pageIndex: 0, width: 600, height: 800, rotation: 0, overlays: [] };
function text(value: string): PageText {
  let x = 10, y = 20;
  return { characters: Array.from(value, (character) => {
    const box = { text: character, x, y, width: character === "\n" ? 0 : 5, height: 10 };
    if (character === "\n") { x = 10; y += 20; } else x += 5;
    return box;
  }) };
}
afterEach(() => { cleanup(); clearSearchTextCache(["source-search", "other-source"]); vi.restoreAllMocks(); });

describe("document search mapping", () => {
  it("matches case-insensitive literal phrases across normalized whitespace and keeps line boxes", () => {
    const matches = findPageMatches(page, text("Alpha  \nBETA a.b axb"), " ALPHA\tbeta ");
    expect(matches).toEqual([{ id: "page-a:source:0", pageId: "page-a", rects: [{ x: 10, y: 20, width: 25, height: 10 }, { x: 10, y: 40, width: 20, height: 10 }] }]);
    expect(findPageMatches(page, text("a.b axb"), "a.b")).toHaveLength(1);
  });
  it("keeps crop-normalized glyph rectangles unchanged on intrinsically rotated source pages", () => {
    const matches = findPageMatches({ ...page, rotation: 270 }, { intrinsicRotation: 90, characters: [
      { text: "A", x: 200, y: 30, width: 10, height: 5 }, { text: "b", x: 200, y: 35, width: 10, height: 5 },
    ] }, "ab");
    expect(matches[0].rects).toEqual([{ x: 200, y: 30, width: 10, height: 10 }]);
  });
  it("searches added text and assigns distinct stable identities to duplicate source pages", () => {
    const original = { ...page, overlays: [{ id: "caption", type: "text" as const, text: "New NOTE", x: 30, y: 90, fontSize: 10, color: "#000000" }] };
    const matches = findPageMatches(original, text("note"), "note");
    expect(matches).toHaveLength(2);
    expect(matches[1].rects[0].x).toBeCloseTo(52.4);
    expect(matches[1].rects[0].width).toBeCloseTo(22.4);
    expect(matches[1].rects[0]).toMatchObject({ y: 90, height: 12 });
    const duplicate = findPageMatches({ ...original, id: "page-copy" }, text("note"), "note");
    expect(duplicate.every((match) => match.pageId === "page-copy")).toBe(true);
    expect(duplicate[0].id).not.toBe(matches[0].id);
  });
});

describe("asynchronous document search", () => {
  it("searches reordered and duplicate pages in document order using one extraction per source page", async () => {
    const calls: number[] = [];
    const adapter = { ...createDemoAdapter(), async getPageText(_id: string, index: number) { calls.push(index); return text(index === 0 ? "needle" : "needle needle"); } };
    const pages = [{ ...page, id: "second", pageIndex: 1 }, page, { ...page, id: "duplicate" }];
    const hook = renderHook(() => useDocumentSearch(adapter, pages, "needle", true));
    await waitFor(() => expect(hook.result.current.searching).toBe(false));
    expect(hook.result.current.matches.map((match) => match.pageId)).toEqual(["second", "second", "page-a", "duplicate"]);
    expect(calls).toEqual([1, 0]);
    expect(hook.result.current.hasText).toBe(true);
  });
  it("ignores a late old-tab extraction and clears old matches as soon as the query changes", async () => {
    let finishOld!: (value: PageText) => void;
    const adapter = { ...createDemoAdapter(), getPageText(id: string) { return id === "source-search" ? new Promise<PageText>((resolve) => { finishOld = resolve; }) : Promise.resolve(text("new query")); } };
    const oldPages = [page], newPages = [{ ...page, id: "new-tab", sourceId: "other-source" }];
    const hook = renderHook(({ pages, query, enabled }) => useDocumentSearch(adapter, pages, query, enabled), { initialProps: { pages: oldPages, query: "old", enabled: true } });
    await waitFor(() => expect(finishOld).toBeTypeOf("function"));
    hook.rerender({ pages: newPages, query: "new", enabled: true });
    await waitFor(() => expect(hook.result.current.matches).toHaveLength(1));
    await act(async () => finishOld(text("old old old")));
    expect(hook.result.current.matches.map((match) => match.pageId)).toEqual(["new-tab"]);
    hook.rerender({ pages: newPages, query: "missing", enabled: true });
    expect(hook.result.current.matches).toEqual([]);
    hook.rerender({ pages: newPages, query: "new", enabled: false });
    expect(hook.result.current.matches).toEqual([]);
    expect(hook.result.current.searching).toBe(false);
  });
  it("distinguishes an image-only document, no matches, and a failed page without hiding other results", async () => {
    const adapter = { ...createDemoAdapter(), async getPageText(_id: string, index: number) { if (index === 1) throw new Error("PDF text unavailable"); return text(index === 0 ? "" : "visible text"); } };
    const empty = [page];
    const hook = renderHook(({ pages, query }) => useDocumentSearch(adapter, pages, query, true), { initialProps: { pages: empty, query: "absent" } });
    await waitFor(() => expect(hook.result.current.searching).toBe(false));
    expect(hook.result.current).toMatchObject({ hasText: false, error: null, matches: [] });
    hook.rerender({ pages: [{ ...page, pageIndex: 2 }], query: "absent" });
    await waitFor(() => expect(hook.result.current.searching).toBe(false));
    expect(hook.result.current).toMatchObject({ hasText: true, error: null, matches: [] });
    hook.rerender({ pages: [{ ...page, pageIndex: 1 }, { ...page, id: "readable", pageIndex: 2 }], query: "visible" });
    await waitFor(() => expect(hook.result.current.searching).toBe(false));
    expect(hook.result.current.error).toContain("PDF text unavailable");
    expect(hook.result.current.matches.map((match) => match.pageId)).toEqual(["readable"]);
    expect(hook.result.current.hasText).toBe(true);
  });
  it("bounds the shared extraction cache and retries failures", async () => {
    const calls: number[] = [];
    let failed = false;
    const adapter = { ...createDemoAdapter(), async getPageText(_id: string, index: number) { calls.push(index); if (index === 99 && !failed) { failed = true; throw new Error("retry"); } return text("cached"); } };
    for (let index = 0; index < 65; index++) await readSearchPageText(adapter, page.sourceId, index);
    await readSearchPageText(adapter, page.sourceId, 64);
    await readSearchPageText(adapter, page.sourceId, 0);
    expect(calls.filter((index) => index === 64)).toHaveLength(1);
    expect(calls.filter((index) => index === 0)).toHaveLength(2);
    await expect(readSearchPageText(adapter, page.sourceId, 99)).rejects.toThrow("retry");
    await expect(readSearchPageText(adapter, page.sourceId, 99)).resolves.toEqual(text("cached"));
  });
});

it("search bar exposes count, keyboard navigation, close, and distinct empty states", () => {
  const actions: string[] = [];
  const props = { query: "needle", onQueryChange: (value: string) => actions.push(value), index: 1, total: 3, searching: false, error: null, hasText: true, onNext: () => actions.push("next"), onPrevious: () => actions.push("previous"), onClose: () => actions.push("close") };
  const view = render(<SearchBar {...props} />);
  expect(screen.getByText("2 of 3")).toBeInTheDocument();
  const input = screen.getByRole("searchbox", { name: "Find in document" });
  expect(input).toHaveFocus();
  fireEvent.keyDown(input, { key: "Enter" });
  fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
  fireEvent.keyDown(input, { key: "Escape" });
  expect(actions).toEqual(["next", "previous", "close"]);
  view.rerender(<SearchBar {...props} total={0} hasText={false} />);
  expect(screen.getByText("No searchable text")).toBeInTheDocument();
  view.rerender(<SearchBar {...props} total={0} />);
  expect(screen.getByText("No matches")).toBeInTheDocument();
});

it("highlights every match once inside the rotated page and centers the active rectangle", async () => {
  const adapter = { ...createDemoAdapter(), renderPage: async () => "data:image/svg+xml,%3Csvg/%3E", getPageText: async () => ({ characters: [] }) };
  const rotatedPage = { ...page, rotation: 90 as const };
  const matches = [{ id: "active", pageId: page.id, rects: [{ x: 100, y: 600, width: 50, height: 20 }] }, { id: "other", pageId: page.id, rects: [{ x: 300, y: 100, width: 30, height: 10 }] }];
  const callbacks = { adapter, pages: [rotatedPage], selectedPageId: page.id, selectedOverlayId: null, zoom: 100, onZoomChange() {}, viewMode: "continuous" as const, tool: "select" as const, penColor: "#000000", penWidth: 2, pendingSignature: null, interactionDisabled: false, onSelectPage() {}, onSelectOverlay() {}, onAddText() {}, onPlaceSignature() {}, onMoveOverlay() {}, onDraw() {} };
  function box(left: number, top: number, width: number, height: number): DOMRect { return { x: left, y: top, left, top, right: left + width, bottom: top + height, width, height, toJSON() {} }; }
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    if (this.classList.contains("document-viewport")) return box(0, 0, 200, 150);
    if (this.classList.contains("document-page")) { const host = this.closest(".document-viewport")!; return box(42 - host.scrollLeft, 38 - host.scrollTop, 800, 600); }
    return box(0, 0, 0, 0);
  });
  const view = render(<DocumentViewport {...callbacks} navigationRequest={null} searchMatches={matches} activeSearchMatchId="active" />);
  expect(view.container.querySelectorAll(".search-highlight")).toHaveLength(2);
  expect(view.container.querySelectorAll(".search-highlight.active")).toHaveLength(1);
  expect(view.container.querySelector(".search-highlights")?.parentElement).toHaveAttribute("transform", "translate(800 0) rotate(90)");
  view.rerender(<DocumentViewport {...callbacks} navigationRequest={{ pageId: page.id, revision: 1, rect: matches[0].rects[0] }} searchMatches={matches} activeSearchMatchId="active" />);
  const host = screen.getByRole("main", { name: "Document" });
  expect(host.scrollLeft).toBe(132);
  expect(host.scrollTop).toBe(88);
});
it("bounds dense glyph memory independently of page count", async () => {
  const glyph = { text: "A", x: 10, y: 20, width: 5, height: 10 };
  const calls: number[] = [];
  const adapter = { ...createDemoAdapter(), async getPageText(_id: string, index: number) { calls.push(index); return { characters: Array.from({ length: 130_000 }, () => glyph) }; } };
  await readSearchPageText(adapter, page.sourceId, 0);
  await readSearchPageText(adapter, page.sourceId, 1);
  await readSearchPageText(adapter, page.sourceId, 1);
  expect(calls).toEqual([0, 1]);
  await readSearchPageText(adapter, page.sourceId, 0);
  expect(calls).toEqual([0, 1, 0]);
});

it("retains recent text after hundreds of pages and never resurrects a closed pending extraction", async () => {
  const getPageText = vi.fn(async (_id: string, index: number) => text(`page ${index}`));
  const adapter = { ...createDemoAdapter(), getPageText };
  for (let index = 0; index < 320; index++) await readSearchPageText(adapter, page.sourceId, index);
  for (let index = 300; index < 320; index++) await readSearchPageText(adapter, page.sourceId, index);
  expect(getPageText).toHaveBeenCalledTimes(320);
  await readSearchPageText(adapter, page.sourceId, 0);
  expect(getPageText).toHaveBeenCalledTimes(321);
  let resolveOld!: (value: PageText) => void;
  const lateAdapter = { ...createDemoAdapter(), getPageText: vi.fn().mockImplementationOnce(() => new Promise<PageText>(resolve => { resolveOld = resolve; })).mockResolvedValue(text("reopened")) };
  const oldRequest = readSearchPageText(lateAdapter, "other-source", 0);
  await Promise.resolve();
  clearSearchTextCache(["other-source"]);
  const reopened = await readSearchPageText(lateAdapter, "other-source", 0);
  resolveOld(text("closed old")); await oldRequest;
  expect(await readSearchPageText(lateAdapter, "other-source", 0)).toBe(reopened);
  expect(lateAdapter.getPageText).toHaveBeenCalledTimes(2);
});