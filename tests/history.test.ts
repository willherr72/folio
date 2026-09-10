import { describe, expect, it } from "vitest";
import { commit, createHistory, duplicatePage, movePage, redo, undo, type EditorDocument } from "../src/editor/model";

const document: EditorDocument = {
  name: "Contract.pdf",
  pages: [
    { id: "page-a", sourceId: "source", pageIndex: 0, width: 612, height: 792, rotation: 0, overlays: [] },
    { id: "page-b", sourceId: "source", pageIndex: 1, width: 612, height: 792, rotation: 0, overlays: [] },
  ],
  selectedPageId: "page-a",
  selectedOverlayId: null,
};

describe("immutable editor history", () => {
  it("restores an edit exactly through undo and redo", () => {
    const original = createHistory(document);
    const edited = commit(original, (current) => ({
      ...current,
      pages: current.pages.map((page) => page.id === "page-a" ? {
        ...page,
        overlays: [{ type: "text" as const, id: "text-1", x: 40, y: 80, text: "Approved", fontSize: 18, color: "#2D2A26" }],
      } : page),
    }));

    expect(edited.present.pages[0].overlays).toHaveLength(1);
    expect(undo(edited).present).toEqual(document);
    expect(redo(undo(edited)).present).toEqual(edited.present);
    expect(original.present).toEqual(document);
  });

  it("clears redo states after a branched edit", () => {
    const once = commit(createHistory(document), (value) => ({ ...value, name: "First" }));
    const branch = commit(undo(once), (value) => ({ ...value, name: "Branch" }));

    expect(branch.future).toEqual([]);
    expect(redo(branch).present.name).toBe("Branch");
  });
});

describe("page identity", () => {
  it("duplicates source content under a new stable plan id", () => {
    const copy = duplicatePage(document, "page-a", "page-copy");

    expect(copy.pages.map((page) => page.id)).toEqual(["page-a", "page-copy", "page-b"]);
    expect(copy.pages[1]).toMatchObject({ sourceId: "source", pageIndex: 0 });
    expect(copy.pages[1].overlays).not.toBe(document.pages[0].overlays);
  });

  it("keeps selection attached to page identity while reordering", () => {
    const moved = movePage({ ...document, selectedPageId: "page-b" }, "page-b", 0);

    expect(moved.pages.map((page) => page.id)).toEqual(["page-b", "page-a"]);
    expect(moved.selectedPageId).toBe("page-b");
  });
});
