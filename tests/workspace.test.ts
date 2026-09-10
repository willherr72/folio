import { describe, expect, it } from "vitest";
import { createDemoAdapter } from "../src/editor/adapter";
import { commit, rotatePage, undo } from "../src/editor/model";
import { addSession, createSession, emptyWorkspace, removeSession, updateSession } from "../src/editor/workspace";
const adapter = createDemoAdapter();
function session(id: string) { return createSession({id, name: id + ".pdf", pages: [{width:612,height:792}]}, adapter, 90); }
describe("document sessions", () => {
  it("keeps each tab history, dirty baseline and zoom independent", () => {
    const a = session("a"), b = session("b");
    let workspace = addSession(addSession(emptyWorkspace(), a), b);
    workspace = updateSession(workspace, a.id, value => ({...value, zoom:130, history:commit(value.history, doc=>rotatePage(doc, doc.pages[0].id))}));
    expect(workspace.tabs[0].history.present.pages[0].rotation).toBe(90);
    expect(workspace.tabs[1].history.past).toHaveLength(0);
    expect(workspace.tabs[1].zoom).toBe(90);
    workspace = updateSession(workspace, a.id, value=>({...value,history:undo(value.history)}));
    expect(workspace.tabs[0].history.present.pages[0].rotation).toBe(0);
    expect(workspace.tabs[0].zoom).toBe(130);
    expect(workspace.tabs[0].savedDigest).toBe(a.savedDigest);
  });
  it("closes only the requested tab and keeps a valid neighbor active", () => {
    const a=session("a"), b=session("b"), c=session("c");
    let workspace=addSession(addSession(addSession(emptyWorkspace(),a),b),c);
    workspace=removeSession(workspace,b.id);
    expect(workspace.activeId).toBe(c.id);
    expect(workspace.tabs.map(tab=>tab.id)).toEqual([a.id,c.id]);
    workspace=removeSession(workspace,c.id);
    expect(workspace.activeId).toBe(a.id);
    expect(removeSession(workspace,a.id)).toEqual(emptyWorkspace());
  });
});
