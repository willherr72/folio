import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../src/App";
import { nativeAdapter } from "../src/editor/adapter";

beforeEach(() => { localStorage.clear(); });
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
async function demo() {
  render(<App initialDemo={false}/>);
  fireEvent.click(screen.getByRole("button", { name: "Explore demo" }));
  await screen.findByText("Folio welcome.pdf");
}
describe("editor upgrades", () => {
  it("uses a cancellable tab-close modal without invoking the native picker or browser confirmation", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const open = vi.spyOn(nativeAdapter, "openPdf").mockResolvedValue(null);
    await demo();
    fireEvent.click(screen.getByRole("button", { name: "Rotate" }));
    fireEvent.click(screen.getByRole("button", { name: "Close Folio welcome.pdf" }));
    const dialog = await screen.findByRole("dialog", { name: "Close document?" });
    expect(dialog).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    expect(screen.getByText("90° clockwise")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Keep editing" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(open).not.toHaveBeenCalled();
    expect(confirm).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Unsaved changes")).toBeInTheDocument();
  });
  it("opens another PDF without discarding edits and preserves the current tab when the picker is cancelled", async () => {
    const open = vi.spyOn(nativeAdapter, "openPdf").mockResolvedValue(null);
    await demo();
    fireEvent.click(screen.getByRole("button", { name: "Rotate" }));
    fireEvent.click(screen.getByRole("button", { name: "Open" }));

    await waitFor(() => expect(open).toHaveBeenCalledTimes(1));
    expect(screen.getByText("Folio welcome.pdf")).toBeInTheDocument();
    expect(screen.getByLabelText("Unsaved changes")).toBeInTheDocument();
  });
  it("opens settings without dirtying the document and removes the engine badge", async () => {
    await demo();
    expect(screen.queryByText(/Browser demo · generated pages/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(await screen.findByRole("dialog", { name: "Settings" })).toBeInTheDocument();
    expect(screen.queryByLabelText("Unsaved changes")).not.toBeInTheDocument();
  });
  it("exposes drawing and stacks all three pages by default", async () => {
    await demo();
    fireEvent.click(screen.getByRole("button", { name: "Draw" }));
    expect(screen.getByRole("button", { name: "Draw" })).toHaveClass("active");
    expect(screen.getByLabelText("Pen color")).toBeInTheDocument();
    expect(document.querySelectorAll("[data-page-id]")).toHaveLength(3);
  });
});
