import {expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {nativeAdapter} from "../src/editor/adapter";
import {createStyledEditor,styledEditorRequest} from "../src/editor/styled-paragraph-editor";
import {styledEditorFixture} from "./fixtures/styled-editor-data";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
it("sends paragraph inspections, fitting and full-source previews through the native bridge",async()=>{
 const ipc=vi.mocked(invoke),selection=styledEditorFixture(),request=styledEditorRequest(createStyledEditor(selection));
 for(const [method,command] of [[nativeAdapter.inspectOccurrenceTextParagraph!,"inspect_occurrence_text_paragraph"],[nativeAdapter.inspectOccurrenceStyledParagraph!,"inspect_occurrence_styled_paragraph"]] as const){
  ipc.mockResolvedValue(selection);expect(await method("source",2,[3,4])).toEqual(selection);
  expect(ipc).toHaveBeenLastCalledWith(command,{sourceId:"source",pageIndex:2,objectIndices:[3,4]});
 }
 for(const [method,command] of [[nativeAdapter.suggestOccurrenceTextParagraph!,"suggest_occurrence_text_paragraph"],[nativeAdapter.suggestOccurrenceStyledParagraph!,"suggest_occurrence_styled_paragraph"]] as const){
  await method("source",2,3,"before");expect(ipc).toHaveBeenLastCalledWith(command,{sourceId:"source",pageIndex:2,objectIndex:3,expectedText:"before"});
 }
 ipc.mockResolvedValue(selection.bounds);expect(await nativeAdapter.fitOccurrenceStyledParagraph!("source",2,request)).toEqual(selection.bounds);
 expect(ipc).toHaveBeenLastCalledWith("fit_occurrence_styled_paragraph",{sourceId:"source",pageIndex:2,request});
 const result={document:{id:"result",name:"Edited.pdf",pages:[]},pageIndices:[0,2,1]};ipc.mockResolvedValue(result);
 expect(await nativeAdapter.previewStyledParagraphOccurrence!("source",[0,0,1],1,request)).toEqual(result);
 expect(ipc).toHaveBeenLastCalledWith("preview_styled_paragraph_occurrence",{sourceId:"source",occurrences:[0,0,1],selectedOccurrence:1,request});
 const args=["source",[0,0,1] as number[],1,[3,4] as number[],"before","after",selection.bounds] as const;
 await nativeAdapter.previewTextParagraphOccurrence!(...args);
 const base={sourceId:"source",occurrences:[0,0,1],selectedOccurrence:1,objectIndices:[3,4],expectedText:"before",replacement:"after",bounds:selection.bounds};
 expect(ipc).toHaveBeenLastCalledWith("preview_text_paragraph_occurrence",base);
 await nativeAdapter.previewTextParagraphOccurrence!(...args,18,"right",1.5,"font","#123456");
 expect(ipc).toHaveBeenLastCalledWith("preview_text_paragraph_occurrence",{...base,fontSize:18,alignment:"right",lineSpacing:1.5,fontId:"font",color:"#123456"});
});
