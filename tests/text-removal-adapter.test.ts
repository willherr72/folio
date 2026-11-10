import {beforeEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {nativeAdapter} from '../src/editor/adapter';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn()}));
beforeEach(()=>{vi.mocked(invoke).mockReset();});
it('exposes source text removal without taking native owner credentials',async()=>{
 expect(nativeAdapter.beginTextRemovalSession).toBeTypeOf('function');
 const ipc=vi.mocked(invoke);ipc.mockResolvedValue('session');
 expect(await nativeAdapter.beginTextRemovalSession!()).toBe('session');
 expect(ipc).toHaveBeenLastCalledWith('begin_text_removal_session');
 const marks=[{pageIndex:2,rect:{x:10,y:20,width:30,height:40}}];
 const result={previewId:'preview',sourceId:'source',candidate:{id:'candidate',name:'Copy.pdf',pages:[]},removedCharacters:6,affectedBlocks:1,omittedMetadata:[],expiresInSeconds:900,pages:[]};
 ipc.mockResolvedValue(result);
 expect(await nativeAdapter.previewSourceTextRemoval!('session','source',marks)).toBe(result);
 expect(ipc).toHaveBeenLastCalledWith('preview_source_text_removal',{sessionId:'session',sourceId:'source',marks});
});
it('scopes paginated bounds, one-shot Apply, discard and release to a session',async()=>{
 expect(nativeAdapter.textRemovalPreviewBounds).toBeTypeOf('function');
 const ipc=vi.mocked(invoke);const bounds={pageIndex:2,offset:10,total:20,boxes:[]};ipc.mockResolvedValue(bounds);
 expect(await nativeAdapter.textRemovalPreviewBounds!('session','preview',2,10,5)).toBe(bounds);
 expect(ipc).toHaveBeenLastCalledWith('text_removal_preview_bounds',{sessionId:'session',previewId:'preview',pageIndex:2,offset:10,limit:5});
 const document={id:'applied',name:'Copy.pdf',pages:[]};ipc.mockResolvedValue(document);
 expect(await nativeAdapter.applyTextRemovalPreview!('session','preview')).toBe(document);
 expect(ipc).toHaveBeenLastCalledWith('apply_text_removal_preview',{sessionId:'session',previewId:'preview'});
 await nativeAdapter.discardTextRemovalPreview!('session','preview');
 expect(ipc).toHaveBeenLastCalledWith('discard_text_removal_preview',{sessionId:'session',previewId:'preview'});
 await nativeAdapter.releaseTextRemovalSession!('session');
 expect(ipc).toHaveBeenLastCalledWith('release_text_removal_session',{sessionId:'session'});
});
it('propagates refusal without replacing the source or claiming Apply',async()=>{
 expect(nativeAdapter.previewSourceTextRemoval).toBeTypeOf('function');
 const ipc=vi.mocked(invoke);ipc.mockRejectedValue(new Error('unsupported source'));
 await expect(nativeAdapter.previewSourceTextRemoval!('session','source',[])).rejects.toThrow('unsupported source');
 expect(ipc).toHaveBeenCalledTimes(1);
});
it('prepares the edited page plans and preserves inherited versus explicitly empty bookmarks',async()=>{
 expect(nativeAdapter.prepareTextRemovalSource).toBeTypeOf('function');
 const ipc=vi.mocked(invoke);
 const pages=[{id:'page',sourceId:'changed-source',pageIndex:2,width:500,height:300,rotation:90 as const,overlays:[],crop:{x:10,y:20,width:200,height:100}}];
 const prepared={document:{id:'working-copy',name:'Working copy.pdf',pages:[]},omittedMetadata:['document information and identifiers (Save composition)']};
 ipc.mockResolvedValue(prepared);
 expect(await nativeAdapter.prepareTextRemovalSource!('session',pages)).toBe(prepared);
 expect(ipc).toHaveBeenLastCalledWith('prepare_text_removal_source',{sessionId:'session',request:{pages,flatten:false}});
 expect(await nativeAdapter.prepareTextRemovalSource!('session',pages,[])).toBe(prepared);
 expect(ipc).toHaveBeenLastCalledWith('prepare_text_removal_source',{sessionId:'session',request:{pages,flatten:false},bookmarks:[]});
 expect(ipc).toHaveBeenCalledTimes(2);
});
