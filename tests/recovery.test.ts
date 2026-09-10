import {describe,it,expect,vi,afterEach} from "vitest";
import {snapshotWorkspace,restoreWorkspace,RecoveryQueue,type RecoverySnapshot} from "../src/editor/recovery";
import {createSession,addSession,emptyWorkspace} from "../src/editor/workspace";
import {nativeAdapter,createDemoAdapter} from "../src/editor/adapter";
const info={id:"source",name:"One.pdf",pages:[{width:300,height:400}]};
function sample():RecoverySnapshot {
 const tab=createSession(info,nativeAdapter,110);
 tab.history.present.pages[0].rotation=90;
 return snapshotWorkspace(addSession(emptyWorkspace(),tab),new Map([[tab.id,{top:240,left:12}]]));
}
afterEach(()=>vi.useRealTimers());
describe("recovery",()=>{
 it("preserves editable state, dirty flag and view without storing demo sessions",()=>{
  const tab=createSession(info,nativeAdapter,110);
  tab.history.present.pages[0].rotation=90;
  const workspace=addSession(addSession(emptyWorkspace(),tab),createSession(info,createDemoAdapter(),90));
  const snapshot=snapshotWorkspace(workspace,new Map([[tab.id,{top:240,left:12}]]));
  expect(snapshot.tabs).toHaveLength(1);
  expect(snapshot.activeId).toBe(tab.id);
  expect(snapshot.tabs[0]).toMatchObject({dirty:true,zoom:110,scrollPosition:{top:240,left:12}});
  expect(snapshot.tabs[0].document.pages[0].rotation).toBe(90);
 });
 it("restores clean/dirty status after native source IDs change and resets undo history",()=>{
  const snapshot=sample();
  snapshot.tabs[0].document.pages[0].sourceId="new-runtime-id";
  const dirty=restoreWorkspace(snapshot,nativeAdapter);
  expect(dirty.workspace.tabs[0].sourceIds).toEqual(["new-runtime-id"]);
  expect(dirty.workspace.tabs[0].history.past).toEqual([]);
  expect(dirty.positions.get(snapshot.tabs[0].id)).toEqual({top:240,left:12});
  const cleanSnapshot=structuredClone(snapshot); cleanSnapshot.tabs[0].dirty=false;
  const clean=restoreWorkspace(cleanSnapshot,nativeAdapter).workspace.tabs[0];
  expect(snapshotWorkspace({tabs:[clean],activeId:clean.id},new Map()).tabs[0].dirty).toBe(false);
  expect(snapshotWorkspace(dirty.workspace,dirty.positions).tabs[0].dirty).toBe(true);
 });
 it("serializes checkpoint writes and clears only after in-flight writes settle",async()=>{
  let finish!:()=>void; const order:string[]=[];
  const queue=new RecoveryQueue({save:async()=>{order.push("save");await new Promise<void>(resolve=>finish=resolve);order.push("saved");},clear:async()=>{order.push("clear");}},()=>{});
  const flush=queue.flush(sample()); await Promise.resolve(); await Promise.resolve();
  const closing=queue.close(); await Promise.resolve();
  expect(order).toEqual(["save"]); finish(); await flush; await closing;
  expect(order).toEqual(["save","saved","clear"]);
  queue.schedule(sample()); await queue.flush();
  expect(order).toEqual(["save","saved","clear"]);
 });
 it("debounces changes but checkpoints continuous editing within five seconds",async()=>{
  vi.useFakeTimers(); const save=vi.fn(async()=>{}); const queue=new RecoveryQueue({save,clear:async()=>{}},()=>{});
  for(let i=0;i<11;i++){queue.schedule(sample()); await vi.advanceTimersByTimeAsync(500);}
  expect(save).toHaveBeenCalledTimes(1);
  queue.dispose();
 });
 it("reports save errors and allows a later successful checkpoint",async()=>{
  const failure=new Error("Disk full");let fail=true;const errors:unknown[]=[];
  const queue=new RecoveryQueue({save:async()=>{if(fail)throw failure;},clear:async()=>{}},error=>errors.push(error));
  await expect(queue.flush(sample())).rejects.toThrow("Disk full");fail=false;await queue.flush(sample());
  expect(errors).toEqual([failure]);
 });
});
