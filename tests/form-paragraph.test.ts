import {expect,it,vi} from "vitest";
import {createDemoAdapter,type FolioAdapter,nativeAdapter} from "../src/editor/adapter";
import {prepareFormParagraph,supportsFormParagraph} from "../src/editor/form-paragraph";
import {styledEditorFixture} from "./fixtures/styled-editor-data";
import {createStyledEditor,styledEditorRequest} from "../src/editor/styled-paragraph-editor";
const anchor={objectIndex:9,objectPath:[2,0,1],text:"one",fontName:"Courier",fontSize:12,bounds:{x:20,y:30,width:50,height:14},supported:true};
const mapped=()=>({...anchor,objectPath:undefined,objectIndex:3});
function adapter():FolioAdapter{return {...createDemoAdapter(),
 listFormParagraphRuns:vi.fn().mockResolvedValue({anchorIndex:3,runs:[mapped()]}),
 inspectFormStyledParagraph:vi.fn().mockResolvedValue(styledEditorFixture()),
 suggestFormStyledParagraph:vi.fn().mockResolvedValue(styledEditorFixture()),
 fitFormStyledParagraph:vi.fn().mockResolvedValue(anchor.bounds),
 previewFormStyledParagraphOccurrence:vi.fn().mockResolvedValue({document:{id:"new",name:"new.pdf",pages:[]},pageIndices:[]})};}
it("binds the complete form contract to its original source, page and immutable path",async()=>{
 const a=adapter(),input={...anchor,objectPath:[...anchor.objectPath]},p=await prepareFormParagraph(a,"source",4,input);
 input.objectPath[0]=99;expect(p.anchor.objectIndex).toBe(3);expect(p.anchor.objectPath).toBeUndefined();
 expect(a.listFormParagraphRuns).toHaveBeenCalledWith("source",4,[2,0,1],"one");
 const runs=await p.adapter.listOccurrenceTextRuns!("source",4);runs.runs[0].bounds.x=999;
 expect((await p.adapter.listOccurrenceTextRuns!("source",4)).runs[0].bounds.x).toBe(20);
 await p.adapter.inspectOccurrenceStyledParagraph!("source",4,[3]);
 expect(a.inspectFormStyledParagraph).toHaveBeenCalledWith("source",4,[2,0,1],[3]);
 await p.adapter.suggestOccurrenceStyledParagraph!("source",4,3,"one");
 expect(a.suggestFormStyledParagraph).toHaveBeenCalledWith("source",4,[2,0,1],3,"one");
 const request=styledEditorRequest(createStyledEditor(styledEditorFixture()));
 await p.adapter.fitOccurrenceStyledParagraph!("source",4,request);
 expect(a.fitFormStyledParagraph).toHaveBeenCalledWith("source",4,[2,0,1],request);
 await p.adapter.previewStyledParagraphOccurrence!("source",[4,0,4],2,request);
 expect(a.previewFormStyledParagraphOccurrence).toHaveBeenCalledWith("source",[4,0,4],2,[2,0,1],request);
 await expect(p.adapter.listOccurrenceTextRuns!("other",4)).rejects.toThrow("source");
 await expect(p.adapter.inspectOccurrenceStyledParagraph!("source",3,[3])).rejects.toThrow("source");
 await expect(p.adapter.previewStyledParagraphOccurrence!("source",[4,0],1,request)).rejects.toThrow("source");
});
it("binds draft mapping to the original reusable object without leaking a projected source",async()=>{
 const a=adapter(),mapping={sourceId:"source",pageIndex:4,objectIndices:[3],anchorIndex:3,paragraphText:"one",sourceText:"one",sourceStart:0,sourceEnd:3,paragraphStart:0,paragraphEnd:3};
 a.mapFormParagraphDraft=vi.fn().mockResolvedValue(mapping);
 const input={...anchor,objectPath:[...anchor.objectPath]},prepared=await prepareFormParagraph(a,"source",4,input);input.objectPath[0]=99;
 expect(await prepared.adapter.mapParagraphDraft!("source",4,[3],"one",3,"one","styled")).toEqual(mapping);
 expect(a.mapFormParagraphDraft).toHaveBeenCalledWith("source",4,[2,0,1],[3],"one",3,"one");
 await expect(prepared.adapter.mapParagraphDraft!("other",4,[3],"one",3,"one","styled")).rejects.toThrow("source");
 await expect(prepared.adapter.mapParagraphDraft!("source",4,[3],"one",3,"one","uniform")).rejects.toThrow("styled");
 expect(a.mapFormParagraphDraft).toHaveBeenCalledTimes(1);
});
it.each(["listFormParagraphRuns","inspectFormStyledParagraph","suggestFormStyledParagraph","fitFormStyledParagraph","previewFormStyledParagraphOccurrence"] as const)("requires %s without legacy fallback",async key=>{
 const a=adapter();a[key]=undefined;expect(supportsFormParagraph(a)).toBe(false);await expect(prepareFormParagraph(a,"source",4,anchor)).rejects.toThrow("unavailable");
});
it.each([
 {anchorIndex:4,runs:[mapped()]},
 {anchorIndex:3,runs:[mapped(),mapped()]},
 {anchorIndex:3,runs:[{...mapped(),text:"different"}]},
 {anchorIndex:3,runs:[{...mapped(),bounds:{...anchor.bounds,x:NaN}}]},
 {anchorIndex:3,runs:[{...mapped(),objectPath:[0,1]}]},
 {anchorIndex:3,runs:[]},
])("rejects malformed mapping without acquiring or closing any source",async response=>{
 const a=adapter();a.closeDocument=vi.fn();vi.mocked(a.listFormParagraphRuns!).mockResolvedValue(response);
 await expect(prepareFormParagraph(a,"source",4,anchor)).rejects.toThrow();expect(a.closeDocument).not.toHaveBeenCalled();
});
it("preserves a native preparation refusal",async()=>{
 const a=adapter();vi.mocked(a.listFormParagraphRuns!).mockRejectedValue(new Error("Clipped text cannot be promoted."));
 await expect(prepareFormParagraph(a,"source",4,anchor)).rejects.toThrow("Clipped text");
});
it("forwards all form paragraph commands with their complete identities",async()=>{
 const invoke=vi.fn().mockResolvedValue({});Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{invoke},configurable:true});
 try{
  await nativeAdapter.listFormParagraphRuns!("s",2,[1,0],"old");
  expect(invoke).toHaveBeenLastCalledWith("list_form_paragraph_runs",{sourceId:"s",pageIndex:2,objectPath:[1,0],expectedText:"old"},undefined);
  await nativeAdapter.inspectFormStyledParagraph!("s",2,[1,0],[4]);
  expect(invoke).toHaveBeenLastCalledWith("inspect_form_styled_paragraph",{sourceId:"s",pageIndex:2,objectPath:[1,0],objectIndices:[4]},undefined);
  await nativeAdapter.suggestFormStyledParagraph!("s",2,[1,0],4,"old");
  expect(invoke).toHaveBeenLastCalledWith("suggest_form_styled_paragraph",{sourceId:"s",pageIndex:2,objectPath:[1,0],objectIndex:4,expectedText:"old"},undefined);
  const request=styledEditorRequest(createStyledEditor(styledEditorFixture()));
  await nativeAdapter.fitFormStyledParagraph!("s",2,[1,0],request);
  expect(invoke).toHaveBeenLastCalledWith("fit_form_styled_paragraph",{sourceId:"s",pageIndex:2,objectPath:[1,0],request},undefined);
  await nativeAdapter.previewFormStyledParagraphOccurrence!("s",[2,2],1,[1,0],request);
  expect(invoke).toHaveBeenLastCalledWith("preview_form_styled_paragraph_occurrence",{sourceId:"s",occurrences:[2,2],selectedOccurrence:1,objectPath:[1,0],request},undefined);
 }finally{delete (window as any).__TAURI_INTERNALS__;}
});
