import {afterEach,expect,it,vi} from 'vitest';
import {invoke} from '@tauri-apps/api/core';
import {pdfSaveApi,startPdfSave} from '../src/editor/pdf-save';
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn().mockResolvedValue(null),Channel:class{onmessage=()=>{};}}));
afterEach(()=>vi.restoreAllMocks());
it('passes security as a transient IPC option outside the persisted page plan',async()=>{
 await pdfSaveApi.start('job',{pages:[],bookmarks:[],security:{mode:'remove'}},vi.fn());
 expect(invoke).toHaveBeenLastCalledWith('save_pdf',expect.objectContaining({jobId:'job',request:{pages:[]},bookmarks:[],security:{mode:'remove'}}));
 await pdfSaveApi.start('ordinary',{pages:[]},vi.fn());expect(vi.mocked(invoke).mock.calls.at(-1)?.[1]).not.toHaveProperty('security');
});
it('accepts native security-stage progress for the owned export',async()=>{
 vi.spyOn(pdfSaveApi,'register').mockResolvedValue('job');vi.spyOn(pdfSaveApi,'cancel').mockResolvedValue(false);vi.spyOn(pdfSaveApi,'start').mockImplementation(async(_id,_req,progress)=>{progress({jobId:'job',stage:'securing',completed:0,total:1});return 'copy.pdf';});
 const progress=vi.fn();await startPdfSave([{id:'p',sourceId:'s',pageIndex:0,width:300,height:400,rotation:0,overlays:[]}],{security:{mode:'remove'}},progress).result;expect(progress).toHaveBeenCalledOnce();
});
