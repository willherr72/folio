import {it,expect} from 'vitest';
import {captureTextRemoval,commitTextRemoval} from '../src/editor/text-removal-workflow';
import {createSession,emptyWorkspace,addSession} from '../src/editor/workspace';
import {createDemoAdapter,nativeAdapter} from '../src/editor/adapter';
import {snapshotWorkspace,restoreWorkspace} from '../src/editor/recovery';
import {undo,redo} from '../src/editor/model';
it('recovery retains an explicit cleared metadata base without relying on source order',()=>{
 const adapter=nativeAdapter,tab=createSession({id:'source',name:'Source.pdf',pages:[{width:300,height:400},{width:300,height:400}]},adapter,100);
 tab.history.present.metadata={mode:'update',clearExisting:true,title:'Public'};tab.history.present.pages.reverse();
 const restored=restoreWorkspace(snapshotWorkspace(addSession(emptyWorkspace(),tab),new Map()),adapter);
 expect(restored.workspace.tabs[0].history.present.metadata).toEqual({mode:'update',clearExisting:true,title:'Public'});expect(restored.workspace.tabs[0].history.present.pages.map(p=>p.pageIndex)).toEqual([1,0]);
});
it('text removal drops pending document metadata and Undo alone restores it',()=>{
 const tab=createSession({id:'source',name:'Source.pdf',pages:[{width:300,height:400}]},createDemoAdapter(),100);
 tab.history.present.metadata={mode:'update',title:'Private title'};const before=tab.history.present;
 const next=commitTextRemoval(tab,captureTextRemoval(tab),{id:'candidate',name:'Removed.pdf',pages:[{width:300,height:400}],bookmarks:[]});
 expect(next.history.present.metadata).toBeUndefined();expect(undo(next.history).present).toBe(before);expect(redo(undo(next.history)).present.metadata).toBeUndefined();
});
