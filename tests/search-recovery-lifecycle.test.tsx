import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { App } from "../src/App";
import { nativeAdapter } from "../src/editor/adapter";
import { clearSearchTextCache } from "../src/editor/search";
import { recoveryApi, snapshotWorkspace, type RecoverySnapshot } from "../src/editor/recovery";
import { addSession, createSession, emptyWorkspace } from "../src/editor/workspace";

const nativeWindow = vi.hoisted(() => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ onCloseRequested: nativeWindow.listen }) }));
beforeEach(() => {
  localStorage.clear();
  Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
  vi.spyOn(recoveryApi, "load").mockResolvedValue(null);
  vi.spyOn(recoveryApi, "save").mockResolvedValue();
  vi.spyOn(recoveryApi, "clear").mockResolvedValue();
  vi.spyOn(nativeAdapter, "openPdf")
    .mockResolvedValueOnce({ id: "lifecycle-one", name: "One.pdf", pages: [{ width: 300, height: 400 }] })
    .mockResolvedValueOnce({ id: "lifecycle-two", name: "Two.pdf", pages: [{ width: 300, height: 400 }] });
  vi.spyOn(nativeAdapter, "renderPage").mockResolvedValue("data:image/png;base64,");
  vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>, "getPageText").mockImplementation(async (source) => ({ characters: Array.from(source === "lifecycle-one" ? "first first second" : "other other", (text, index) => ({ text, x: 10 + index * 6, y: 40, width: 6, height: 12 })) }));
  vi.spyOn(nativeAdapter, "closeDocument").mockResolvedValue();
});
afterEach(() => {
  cleanup();
  clearSearchTextCache(["lifecycle-one", "lifecycle-two", "recovery-old", "recovery-new"]);
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  vi.restoreAllMocks();
});
async function openOne() {
  render(<App initialDemo={false} />);
  await waitFor(() => expect(screen.getByRole("button", { name: "Open a PDF" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Open a PDF" }));
  await screen.findByRole("tab", { name: "One.pdf" });
}
function recovered(sourceId = "recovery-old"): RecoverySnapshot {
  const tab = createSession({ id: sourceId, name: "Recovered.pdf", pages: [{ width: 300, height: 400 }] }, nativeAdapter, 110);
  tab.history.present.pages[0].rotation = 90;
  return snapshotWorkspace(addSession(emptyWorkspace(), tab), new Map());
}

it("retains independent search query, match and visibility per tab and resets index for a new query", async () => {
  await openOne();
  fireEvent.click(screen.getByRole("button", { name: "Find" }));
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "first" } });
  await screen.findByText("1 of 2");
  fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Enter" });
  expect(screen.getByText("2 of 2")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Open" }));
  await screen.findByRole("tab", { name: "Two.pdf" });
  expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Find" }));
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "other" } });
  await screen.findByText("1 of 2");
  fireEvent.click(screen.getByRole("tab", { name: "One.pdf" }));
  expect(screen.getByRole("searchbox")).toHaveValue("first");
  await screen.findByText("2 of 2");
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "second" } });
  await screen.findByText("1 of 1");
  fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Escape" });
  fireEvent.click(screen.getByRole("tab", { name: "Two.pdf" }));
  expect(screen.getByRole("searchbox")).toHaveValue("other");
  await screen.findByText("1 of 2");
  fireEvent.click(screen.getByRole("tab", { name: "One.pdf" }));
  expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Find" }));
  expect(screen.getByRole("searchbox")).toHaveValue("second");
  await screen.findByText("1 of 1");
  expect(screen.queryByLabelText("Unsaved changes")).not.toBeInTheDocument();
});

it("never checkpoints a discarded tab again when scrolling during its delayed close checkpoint", async () => {
  await openOne();
  fireEvent.click(screen.getByRole("button", { name: "Rotate" }));
  const snapshots: RecoverySnapshot[] = [];
  let finishCheckpoint!: () => void;
  vi.mocked(recoveryApi.save).mockImplementation(async (value) => {
    snapshots.push(structuredClone(value));
    if (snapshots.length === 1) await new Promise<void>((resolve) => { finishCheckpoint = resolve; });
  });
  fireEvent.click(screen.getByRole("button", { name: "Close One.pdf" }));
  fireEvent.click(await screen.findByRole("button", { name: "Discard and close" }));
  await waitFor(() => expect(finishCheckpoint).toBeTypeOf("function"));
  const viewport = screen.getByRole("main", { name: "Document" });
  viewport.scrollTop = 120;
  fireEvent.scroll(viewport);
  await act(async () => { await new Promise((resolve) => setTimeout(resolve, 1100)); });
  await act(async () => finishCheckpoint());
  await waitFor(() => expect(screen.queryByRole("tab", { name: "One.pdf" })).not.toBeInTheDocument());
  expect(snapshots.length).toBeGreaterThan(1);
  expect(snapshots.every((snapshot) => snapshot.tabs.length === 0)).toBe(true);
});

it("blocks opens and autosaves for corrupt recovery until a successful retry", async () => {
  vi.mocked(recoveryApi.load).mockRejectedValueOnce(new Error("Recovery source is corrupt"));
  render(<App initialDemo={false} />);
  expect(await screen.findByRole("dialog", { name: "Recovery needs attention" })).toHaveTextContent("Recovery source is corrupt");
  expect(screen.getByRole("button", { name: "Open a PDF" })).toBeDisabled();
  expect(recoveryApi.save).not.toHaveBeenCalled();
  expect(nativeAdapter.openPdf).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  expect(screen.getByRole("button", { name: "Open a PDF" })).toBeEnabled();
  expect(recoveryApi.load).toHaveBeenCalledTimes(2);
});

it("releases pending source handles when failed discard is followed by continuing without recovery", async () => {
  vi.mocked(recoveryApi.load).mockResolvedValueOnce(recovered());
  vi.mocked(recoveryApi.clear).mockRejectedValueOnce(new Error("Checkpoint is locked"));
  render(<App initialDemo={false} />);
  fireEvent.click(await screen.findByRole("button", { name: "Discard recovery" }));
  expect(await screen.findByRole("dialog", { name: "Recovery needs attention" })).toHaveTextContent("Checkpoint is locked");
  expect(nativeAdapter.closeDocument).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Continue without recovery" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("recovery-old");
  expect(recoveryApi.save).not.toHaveBeenCalled();
});

it("releases superseded pending source handles when retry loads a fresh recovery", async () => {
  vi.mocked(recoveryApi.load).mockResolvedValueOnce(recovered()).mockResolvedValueOnce(recovered("recovery-new"));
  vi.mocked(recoveryApi.clear).mockRejectedValueOnce(new Error("Checkpoint is locked"));
  render(<App initialDemo={false} />);
  fireEvent.click(await screen.findByRole("button", { name: "Discard recovery" }));
  fireEvent.click(await screen.findByRole("button", { name: "Retry" }));
  await screen.findByRole("button", { name: "Restore workspace" });
  expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("recovery-old");
  expect(nativeAdapter.closeDocument).not.toHaveBeenCalledWith("recovery-new");
  fireEvent.click(screen.getByRole("button", { name: "Restore workspace" }));
  expect(await screen.findByRole("tab", { name: "Recovered.pdf" })).toBeInTheDocument();
  expect(screen.getByLabelText("Unsaved changes")).toBeInTheDocument();
});

it("keeps the still-open dirty tab in recovery after a failed close checkpoint and concurrent scrolling", async () => {
  await openOne();
  fireEvent.click(screen.getByRole("button", { name: "Rotate" }));
  const snapshots: RecoverySnapshot[] = [];
  let failCheckpoint!: (error: Error) => void;
  vi.mocked(recoveryApi.save).mockImplementation(async (value) => {
    snapshots.push(structuredClone(value));
    if (snapshots.length === 1) await new Promise<void>((_resolve, reject) => { failCheckpoint = reject; });
  });
  fireEvent.click(screen.getByRole("button", { name: "Close One.pdf" }));
  fireEvent.click(await screen.findByRole("button", { name: "Discard and close" }));
  await waitFor(() => expect(failCheckpoint).toBeTypeOf("function"));
  const viewport = screen.getByRole("main", { name: "Document" });
  viewport.scrollTop = 120;
  fireEvent.scroll(viewport);
  await act(async () => failCheckpoint(new Error("Disk is full")));
  expect(await screen.findByRole("alert")).toHaveTextContent("Couldn’t close document: Disk is full");
  expect(screen.getByRole("tab", { name: "One.pdf" })).toBeInTheDocument();
  expect(nativeAdapter.closeDocument).not.toHaveBeenCalled();
  await act(async () => { await new Promise((resolve) => setTimeout(resolve, 1100)); });
  expect(snapshots.length).toBeGreaterThan(1);
  expect(snapshots.at(-1)?.tabs).toHaveLength(1);
  expect(snapshots.at(-1)?.tabs[0].dirty).toBe(true);
});
