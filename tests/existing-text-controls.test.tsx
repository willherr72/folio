import { act, cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PageView, clearRenderCache } from "../src/components/PageView";
import { createDemoAdapter } from "../src/editor/adapter";
import type { EditableTextRun, PagePlan, TextRuns } from "../src/editor/types";

const run: EditableTextRun = { objectIndex: 3, text: "Original", fontName: "Helvetica", fontSize: 12, bounds: { x: 20, y: 30, width: 80, height: 12 }, supported: true };
const page: PagePlan = { id: "existing-page", sourceId: "existing-source", pageIndex: 2, width: 200, height: 300, rotation: 90, overlays: [{ type: "text", id: "annotation", text: "Annotation", x: 20, y: 30, fontSize: 12, color: "#000" }] };
function options() {
  return { adapter: { ...createDemoAdapter(), listTextRuns: vi.fn(async () => ({ runs: [run] })) }, page, pageNumber: 1, zoom: 150, tool: "edit" as const, selectedOverlayId: null, pendingSignature: null, onActivate: vi.fn(), onEditText: vi.fn(), onSelectOverlay: vi.fn(), onAddText: vi.fn(), onPlaceSignature: vi.fn(), onMoveOverlay: vi.fn() };
}
afterEach(() => { cleanup(); clearRenderCache([page.sourceId, "new-source"]); });

describe("existing text hit targets", () => {
  it("loads only in edit mode and selects a run inside the page rotation", async () => {
    const props = options();
    const view = render(<PageView {...props} tool="select" />);
    expect(props.adapter.listTextRuns).not.toHaveBeenCalled();
    view.rerender(<PageView {...props} />);
    const target = await view.findByRole("button", { name: "Edit text: Original" });
    expect(props.adapter.listTextRuns).toHaveBeenCalledWith("existing-source", 2);
    expect(target.closest("g[transform]")).toHaveAttribute("transform", "translate(300 0) rotate(90)");
    expect(target).toHaveAttribute("x", "20");
    expect(target).toHaveAttribute("y", "30");
    fireEvent.keyDown(target, { key: "Enter" });
    expect(props.onEditText).toHaveBeenCalledWith(run);
    expect(props.onActivate).toHaveBeenCalledOnce();
    expect(view.getByLabelText("Page 1 text")).toHaveAttribute("data-selectable", "false");
    fireEvent.keyDown(view.getByRole("button", { name: "Text: Annotation" }), { key: "Enter" });
    expect(props.onSelectOverlay).not.toHaveBeenCalled();
    expect(props.onMoveOverlay).not.toHaveBeenCalled();
  });

  it("ignores late source responses and never leaves stale run targets clickable", async () => {
    let resolveOld!: (value: TextRuns) => void;
    const props = options();
    props.adapter.listTextRuns.mockImplementationOnce(() => new Promise(resolve => { resolveOld = resolve; }));
    const view = render(<PageView {...props} />);
    view.rerender(<PageView {...props} page={{ ...page, sourceId: "new-source", pageIndex: 0 }} />);
    await view.findByRole("button", { name: "Edit text: Original" });
    await act(async () => resolveOld({ runs: [{ ...run, text: "Stale" }] }));
    expect(view.queryByRole("button", { name: "Edit text: Stale" })).not.toBeInTheDocument();
    expect(props.adapter.listTextRuns).toHaveBeenLastCalledWith("new-source", 0);
  });

  it("allows inspection of unsupported runs but blocks all disabled interactions", async () => {
    const props = options();
    const unsupported = { ...run, supported: false, reason: "Embedded font cannot be preserved." };
    props.adapter.listTextRuns.mockResolvedValue({ runs: [unsupported] });
    const view = render(<PageView {...props} />);
    const target = await view.findByRole("button", { name: /Original/ });
    fireEvent.click(target);
    expect(props.onEditText).toHaveBeenCalledWith(unsupported);
    props.onEditText.mockClear();
    props.onActivate.mockClear();
    view.rerender(<PageView {...props} interactionDisabled />);
    fireEvent.click(target);
    fireEvent.keyDown(target, { key: " " });
    expect(props.onEditText).not.toHaveBeenCalled();
    expect(props.onActivate).not.toHaveBeenCalled();
    expect(target).toHaveAttribute("tabindex", "-1");
  });

  it("explains empty scanned pages and reports extraction failures", async () => {
    const props = options();
    props.adapter.listTextRuns.mockResolvedValue({ runs: [] });
    const view = render(<PageView {...props} />);
    expect(await view.findByRole("status")).toHaveTextContent(/No editable text.*scanned/i);
    props.adapter.listTextRuns.mockRejectedValue(new Error("Page extraction failed"));
    view.rerender(<PageView {...props} page={{ ...page, sourceId: "new-source" }} />);
    await waitFor(() => expect(view.getByRole("status")).toHaveTextContent("Page extraction failed"));
  });
});
