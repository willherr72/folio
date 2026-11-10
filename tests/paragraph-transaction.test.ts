import {expect,it,vi} from "vitest";
import {createDemoAdapter,type FolioAdapter} from "../src/editor/adapter";
import {paragraphEditorKind,paragraphReadAdapter,previewTextParagraph,previewStyledParagraph} from "../src/editor/paragraph-transaction";
import {createStyledEditor,styledEditorRequest} from "../src/editor/styled-paragraph-editor";
import {styledEditorFixture} from "./fixtures/styled-editor-data";
import type {EditorDocument,PagePlan} from "../src/editor/types";
const size={width:400,height:500},bounds={x:20,y:30,width:100,height:50};
const page:PagePlan={id:"p",sourceId:"source",pageIndex:1,...size,rotation:90,overlays:[]};
const document:EditorDocument={name:"Test.pdf",pages:[{...page,id:"other",sourceId:"other-source"},page],selectedPageId:"p",selectedOverlayId:null};
const occurrence={document,page,isSourceRetained:()=>false};
const fresh=()=>({id:"fresh",name:"Preview.pdf",pages:[size,size]});
function adapter():FolioAdapter{return {...createDemoAdapter(),closeDocument:vi.fn().mockResolvedValue(undefined),listTextRuns:vi.fn(),inspectTextParagraph:vi.fn(),previewTextParagraph:vi.fn(),inspectStyledParagraph:vi.fn(),suggestStyledParagraph:vi.fn(),previewStyledParagraph:vi.fn(),fitStyledParagraph:vi.fn(),listOccurrenceTextRuns:vi.fn(),inspectOccurrenceTextParagraph:vi.fn(),previewTextParagraphOccurrence:vi.fn().mockResolvedValue({document:fresh(),pageIndices:[1]})};}
it("prefers a complete native uniform capability over legacy styled, with no mixed inspection or fitting",()=>{
 const a=adapter();expect(paragraphEditorKind(a)).toBe("uniform");
 const uniform=paragraphReadAdapter(a,occurrence,"uniform");expect(uniform.listTextRuns).toBe(a.listOccurrenceTextRuns);expect(uniform.inspectTextParagraph).toBe(a.inspectOccurrenceTextParagraph);expect(uniform.suggestTextParagraph).toBeUndefined();
 a.inspectOccurrenceStyledParagraph=vi.fn();a.suggestOccurrenceStyledParagraph=vi.fn();a.previewStyledParagraphOccurrence=vi.fn();
 expect(paragraphEditorKind(a)).toBe("styled");expect(paragraphReadAdapter(a,occurrence,"styled").fitStyledParagraph).toBeUndefined();
 a.previewStyledParagraphOccurrence=undefined;a.previewTextParagraphOccurrence=undefined;
 expect(paragraphEditorKind(a)).toBe("styled");expect(paragraphReadAdapter(a,occurrence,"styled")).toBe(a);
});
it("maps mixed-source selections, releases once, and does not fall back after native refusal",async()=>{
 const a=adapter();const value=await previewTextParagraph(a,occurrence,"source",1,[2],"before","after",bounds);
 expect(a.previewTextParagraphOccurrence).toHaveBeenCalledWith("source",[1],0,[2],"before","after",bounds);expect(value.pageIndex).toBe(1);
 await value.release();await value.release();expect(a.closeDocument).toHaveBeenCalledTimes(1);
 vi.mocked(a.previewTextParagraphOccurrence!).mockRejectedValueOnce(new Error("Native refused"));
 await expect(previewTextParagraph(a,occurrence,"source",1,[2],"before","after",bounds)).rejects.toThrow("Native refused");expect(a.previewTextParagraph).not.toHaveBeenCalled();
});
it("does not release a candidate acquired by the workspace while rendering",async()=>{
 const a=adapter();let retained=false;const value=await previewTextParagraph(a,{...occurrence,isSourceRetained:()=>retained},"source",1,[2],"before","after",bounds);retained=true;await value.release();expect(a.closeDocument).not.toHaveBeenCalled();
});
it("rejects a stale frozen selection before submitting any native preview",async()=>{
 const a=adapter();await expect(previewTextParagraph(a,{...occurrence,page:{...page}},"source",1,[2],"before","after",bounds)).rejects.toThrow();expect(a.previewTextParagraphOccurrence).not.toHaveBeenCalled();
});
it("validates legacy preview dimensions and borrowed ownership before adopting a candidate",async()=>{
 const a=adapter(),request=styledEditorRequest(createStyledEditor(styledEditorFixture()));
 vi.mocked(a.previewStyledParagraph!).mockResolvedValueOnce(fresh());
 await expect(previewStyledParagraph(a,occurrence,"source",1,request)).rejects.toThrow("dimensions");expect(a.closeDocument).toHaveBeenCalledWith("fresh");
 vi.mocked(a.previewStyledParagraph!).mockResolvedValueOnce({...fresh(),id:"other-source",pages:[size]});
 await expect(previewStyledParagraph(a,occurrence,"source",1,request)).rejects.toThrow("new source");expect(a.closeDocument).not.toHaveBeenCalledWith("other-source");
});

it("holds a discarded candidate until its last raster read settles",async()=>{
 const a=adapter(),value=await previewTextParagraph(a,occurrence,"source",1,[2],"before","after",bounds);
 const first=value.retain(),second=value.retain();await value.release();expect(a.closeDocument).not.toHaveBeenCalled();
 await first();await first();expect(a.closeDocument).not.toHaveBeenCalled();await second();expect(a.closeDocument).toHaveBeenCalledTimes(1);expect(a.closeDocument).toHaveBeenCalledWith("fresh");
});
