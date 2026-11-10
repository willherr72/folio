import {afterEach,expect,it,vi} from 'vitest';
import {textExportSelection,startTextExport,textExportApi} from '../src/editor/text-export';
import type {PagePlan} from '../src/editor/types';
const pages:PagePlan[]=[0,1,2].map(i=>({id:`p${i}`,sourceId:'s',pageIndex:i,width:300,height:400,rotation:0,overlays:[]}));
afterEach(()=>vi.restoreAllMocks());
it('selects distinct pages in workspace order and enforces copy permission',()=>{
 expect(textExportSelection(pages,'range','p0','3,1-2,1',true).pages).toEqual(pages);
 expect(textExportSelection(pages,'current','p1','',false)).toEqual({pages:[pages[1]],includeFormValues:false});
 const denied={...pages[1],protection:{documentId:'rights',revision:6,owner:false,permissions:{copyContent:false,print:true,printHighQuality:true,editContent:true,annotate:true,fillForms:true,assemble:true,accessibility:true}}};
 expect(()=>textExportSelection([pages[0],denied],'all','p0','',true)).toThrow(/copying/);
 expect(textExportSelection([pages[0],denied],'current','p0','',true).pages).toEqual([pages[0]]);
});
it('cancels before registration resolves and never starts a cancelled export',async()=>{
 let resolve!:(id:string)=>void;vi.spyOn(textExportApi,'register').mockImplementation(()=>new Promise(r=>{resolve=r;}));
 const cancel=vi.spyOn(textExportApi,'cancel').mockResolvedValue(true),start=vi.spyOn(textExportApi,'start');
 const task=startTextExport({pages,includeFormValues:false},vi.fn());await task.cancel();resolve('job');expect(await task.result).toBeNull();expect(start).not.toHaveBeenCalled();expect(cancel).toHaveBeenCalledWith('job');
});
it('ignores foreign and late progress and releases registration after failure',async()=>{
 vi.spyOn(textExportApi,'register').mockResolvedValue('ours');const cancel=vi.spyOn(textExportApi,'cancel').mockResolvedValue(true);const progress=vi.fn();
 vi.spyOn(textExportApi,'start').mockImplementation(async(_id,_request,report)=>{report({jobId:'foreign',completed:1,total:3,stage:'extracting'});report({jobId:'ours',completed:1,total:3,stage:'extracting'});throw Error('Export failed');});
 await expect(startTextExport({pages,includeFormValues:false},progress).result).rejects.toThrow('Export failed');expect(progress).toHaveBeenCalledTimes(1);expect(cancel).toHaveBeenCalledWith('ours');
});
