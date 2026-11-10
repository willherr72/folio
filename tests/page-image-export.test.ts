import {expect,it,vi,afterEach} from "vitest";
import {imageExportSelection,imageExportApi,startPageImageExport} from "../src/editor/page-image-export";
import type {PagePlan} from "../src/editor/types";
const page=(id:string):PagePlan=>({id,sourceId:"source",pageIndex:0,width:300,height:400,rotation:0,overlays:[]});
const options={format:"png" as const,dpi:150 as const,jpegQuality:90};
afterEach(()=>vi.restoreAllMocks());
it("uses workspace numbers and visible physical dimensions for the selected occurrences",()=>{
 const pages=[page("a"),{...page("b"),rotation:90 as const,crop:{x:20,y:40,width:100,height:150}},page("c")];
 const result=imageExportSelection(pages,"range","a","3,2-3",options);
 expect(result.request.pageNumbers).toEqual([2,3]);expect(result.request.pages.map(p=>p.id)).toEqual(["b","c"]);expect(result.firstSize).toEqual({width:313,height:208});expect(result.request.workspacePageCount).toBe(3);
});
it("checks only selected copy permissions and rejects oversized output instead of lowering DPI",()=>{
 const denied={...page("b"),protection:{permissions:{copyContent:false}}} as PagePlan;
 expect(()=>imageExportSelection([page("a"),denied],"current","a","",options)).not.toThrow();
 expect(()=>imageExportSelection([page("a"),denied],"all","a","",options)).toThrow(/copy/i);
 expect(()=>imageExportSelection([{...page("a"),width:1200,height:1500}],"all","a","",{...options,dpi:600})).toThrow(/lower.*DPI/i);
});
it("cancels a registration that finishes after unmount without starting it",async()=>{
 let resolve!:(id:string)=>void;vi.spyOn(imageExportApi,"register").mockImplementation(()=>new Promise(r=>{resolve=r;}));const cancel=vi.spyOn(imageExportApi,"cancel").mockResolvedValue(true);const start=vi.spyOn(imageExportApi,"start").mockResolvedValue(null);
 const request=imageExportSelection([page("a")],"all","a","",options).request;const task=startPageImageExport(request,vi.fn());await task.cancel();resolve("job");expect(await task.result).toBeNull();expect(cancel).toHaveBeenCalledWith("job");expect(start).not.toHaveBeenCalled();
});
it("ignores stale progress and respects a completed native result after late cancellation",async()=>{
 vi.spyOn(imageExportApi,"register").mockResolvedValue("job");vi.spyOn(imageExportApi,"cancel").mockResolvedValue(false);let finish!:(v:any)=>void;let report!:(v:any)=>void;
 vi.spyOn(imageExportApi,"start").mockImplementation((_id,_request,progress)=>{report=progress;return new Promise(r=>{finish=r;});});
 const progress=vi.fn(),task=startPageImageExport(imageExportSelection([page("a")],"all","a","",options).request,progress);await Promise.resolve();
 report({jobId:"old",stage:"writing",completed:1,total:1});expect(progress).not.toHaveBeenCalled();report({jobId:"job",stage:"writing",completed:1,total:1});expect(progress).toHaveBeenCalledOnce();await task.cancel();finish({folder:"done",count:1});expect(await task.result).toEqual({folder:"done",count:1});report({jobId:"job",stage:"completed",completed:1,total:1});expect(progress).toHaveBeenCalledOnce();
});
