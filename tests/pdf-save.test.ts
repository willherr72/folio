import {afterEach,expect,it,vi} from 'vitest';
import {pdfSaveApi,startPdfSave,type PdfSaveProgress} from '../src/editor/pdf-save';
import type {PagePlan} from '../src/editor/types';
const pages:PagePlan[]=[{id:'p',sourceId:'s',pageIndex:0,width:300,height:400,rotation:0,overlays:[]}];
afterEach(()=>vi.restoreAllMocks());

it('cancels before registration without starting any save',async()=>{
 let registered!:(id:string)=>void;vi.spyOn(pdfSaveApi,'register').mockImplementation(()=>new Promise(r=>{registered=r;}));
 const cancel=vi.spyOn(pdfSaveApi,'cancel').mockResolvedValue(true),start=vi.spyOn(pdfSaveApi,'start');
 const work=startPdfSave(pages,{},vi.fn());expect(await work.cancel()).toBe(true);registered('ours');
 expect(await work.result).toBeNull();expect(start).not.toHaveBeenCalled();expect(cancel).toHaveBeenCalledWith('ours');
});
it('keeps successful publication when cancellation arrives too late',async()=>{
 vi.spyOn(pdfSaveApi,'register').mockResolvedValue('ours');vi.spyOn(pdfSaveApi,'cancel').mockResolvedValue(false);
 let complete!:(path:string)=>void;vi.spyOn(pdfSaveApi,'start').mockImplementation(()=>new Promise(r=>{complete=r;}));
 const work=startPdfSave(pages,{},vi.fn());await vi.waitFor(()=>expect(complete).toBeTypeOf('function'));
 expect(await work.cancel()).toBe(false);complete('saved.pdf');expect(await work.result).toBe('saved.pdf');
 expect(await work.cancel()).toBe(false);
});
it('resumes progress and allows retry after cancellation request failure',async()=>{
 vi.spyOn(pdfSaveApi,'register').mockResolvedValue('ours');const cancel=vi.spyOn(pdfSaveApi,'cancel').mockRejectedValueOnce(Error('IPC unavailable')).mockResolvedValue(true);
 let report!:(p:PdfSaveProgress)=>void,complete!:(path:null)=>void;
 vi.spyOn(pdfSaveApi,'start').mockImplementation((_id,_request,onProgress)=>{report=onProgress;return new Promise(r=>{complete=r;});});
 const progress=vi.fn(),work=startPdfSave(pages,{},progress);await vi.waitFor(()=>expect(report).toBeTypeOf('function'));
 await expect(work.cancel()).rejects.toThrow('IPC unavailable');report({jobId:'ours',stage:'copying',completed:1,total:1});expect(progress).toHaveBeenCalledOnce();
 expect(await work.cancel()).toBe(true);complete(null);expect(await work.result).toBeNull();expect(cancel).toHaveBeenCalledTimes(3);
});
it('ignores foreign, malformed and late progress, preserving explicit save options',async()=>{
 vi.spyOn(pdfSaveApi,'register').mockResolvedValue('ours');vi.spyOn(pdfSaveApi,'cancel').mockResolvedValue(false);
 let report!:(p:PdfSaveProgress)=>void;const start=vi.spyOn(pdfSaveApi,'start').mockImplementation(async(_id,request,onProgress)=>{
  report=onProgress;expect(request).toEqual({pages,flatten:true,bookmarks:[]});
  onProgress({jobId:'other',stage:'copying',completed:0,total:1});
  onProgress({jobId:'ours',stage:'copying',completed:2,total:1});
  onProgress({jobId:'ours',stage:'copying',completed:0,total:2});
  onProgress({jobId:'ours',stage:'unknown' as any,completed:0,total:1});
  onProgress({jobId:'ours',stage:'copying',completed:1,total:1});return 'saved.pdf';
 });
 const progress=vi.fn();expect(await startPdfSave(pages,{flatten:true,bookmarks:[]},progress).result).toBe('saved.pdf');
 expect(start).toHaveBeenCalledOnce();expect(progress).toHaveBeenCalledOnce();report({jobId:'ours',stage:'finishing',completed:1,total:1});expect(progress).toHaveBeenCalledOnce();
});
it('cleans up registration after native save failure',async()=>{
 vi.spyOn(pdfSaveApi,'register').mockResolvedValue('ours');const cancel=vi.spyOn(pdfSaveApi,'cancel').mockResolvedValue(false);
 vi.spyOn(pdfSaveApi,'start').mockRejectedValue(Error('Write failed'));
 await expect(startPdfSave(pages,{},vi.fn()).result).rejects.toThrow('Write failed');expect(cancel).toHaveBeenCalledWith('ours');
});
it('treats an accepted cancel racing the native start as cancellation',async()=>{
 vi.spyOn(pdfSaveApi,'register').mockResolvedValue('ours');vi.spyOn(pdfSaveApi,'cancel').mockResolvedValue(true);
 let fail!:(reason:Error)=>void;vi.spyOn(pdfSaveApi,'start').mockImplementation(()=>new Promise((_r,reject)=>{fail=reject;}));
 const work=startPdfSave(pages,{},vi.fn());await vi.waitFor(()=>expect(fail).toBeTypeOf('function'));
 expect(await work.cancel()).toBe(true);fail(Error('Registration no longer available'));expect(await work.result).toBeNull();
});
