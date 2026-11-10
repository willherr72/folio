import {it,expect,vi,afterEach} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {createSession,addSession,emptyWorkspace} from '../src/editor/workspace';
import {nativeAdapter} from '../src/editor/adapter';
import {planDigest,commit,undo,redo} from '../src/editor/model';
import {restoreWorkspace,snapshotWorkspace} from '../src/editor/recovery';
import {pdfSaveApi} from '../src/editor/pdf-save';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn().mockResolvedValue(null),Channel:class{onmessage:unknown;}}));
afterEach(()=>vi.clearAllMocks());
const source={id:'s',name:'One.pdf',pages:[{width:300,height:400}]};
it('property edits participate in dirty tracking and undo redo without changing pages',()=>{
 const tab=createSession(source,nativeAdapter,100),before=planDigest(tab.history.present);
 const edited=commit(tab.history,d=>({...d,metadata:{mode:'update' as const,title:'Changed'}}));
 expect(planDigest(edited.present)).not.toBe(before);expect(edited.present.pages).toBe(tab.history.present.pages);
 expect(planDigest(undo(edited).present)).toBe(before);expect(planDigest(redo(undo(edited)).present)).toBe(planDigest(edited.present));
});
it('recovery rejects malformed metadata actions instead of adopting an unsafe draft',()=>{
 const tab=createSession(source,nativeAdapter,100);const snapshot=snapshotWorkspace(addSession(emptyWorkspace(),tab),new Map());
 (snapshot.tabs[0].document as any).metadata={mode:'remove',title:'must not be ignored'};
 expect(()=>restoreWorkspace(snapshot,nativeAdapter)).toThrow(/metadata|properties/i);
});
it('controlled and fallback Save IPC carry explicit document property edits',async()=>{
 const tab=createSession(source,nativeAdapter,100),metadata={mode:'update',title:'New'};
 await pdfSaveApi.start('job',{pages:tab.history.present.pages,metadata} as any,vi.fn());
 expect(vi.mocked(invoke)).toHaveBeenLastCalledWith('save_pdf',expect.objectContaining({metadata}));
 await nativeAdapter.exportPdf(tab.history.present.pages,{metadata} as any);
 expect(vi.mocked(invoke)).toHaveBeenLastCalledWith('export_pdf',expect.objectContaining({metadata}));
});
