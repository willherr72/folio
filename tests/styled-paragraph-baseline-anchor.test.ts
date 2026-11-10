import {expect,it} from "vitest";
import {createStyledEditor,editStyledEditor,undoStyledEditor,redoStyledEditor,styledEditorRequest} from "../src/editor/styled-paragraph-editor";
import type {StyledParagraphSelection} from "../src/editor/types";
const source=():StyledParagraphSelection&{baselineAnchor:{x:number;y:number}}=>({
 objectIndices:[0],text:"AA BB",runs:[{text:"AA BB",styleId:0}],styles:[{styleId:0,objectIndices:[0],fontName:"Courier",fontDisplayName:null,fontSize:12,color:"#000000",isEmbedded:false}],
 bounds:{x:20,y:30,width:80,height:20},alignment:"left",lineSpacing:1.2,baselineAnchor:{x:20,y:38}
});
it("retains an immutable baseline through typing and formatting",()=>{
 const input=source();let state=createStyledEditor(input);input.baselineAnchor.y=999;
 state=editStyledEditor(state,{type:"replace",start:0,end:5,text:"AA BB CC"});
 state=editStyledEditor(state,{type:"select",start:0,end:8});state=editStyledEditor(state,{type:"format",patch:{fontSize:18}});
 expect(styledEditorRequest(state)).toMatchObject({baselineAnchor:{x:20,y:38}});
});
it("translates the baseline with position and restores it with undo and redo",()=>{
 const original=createStyledEditor(source());const moved=editStyledEditor(original,{type:"area",bounds:{...original.present.bounds,x:25,y:40}});
 expect(styledEditorRequest(moved)).toMatchObject({baselineAnchor:{x:25,y:48}});
 expect(styledEditorRequest(undoStyledEditor(moved))).toMatchObject({baselineAnchor:{x:20,y:38}});
 expect(styledEditorRequest(redoStyledEditor(undoStyledEditor(moved)))).toEqual(styledEditorRequest(moved));
});
it("keeps baseline fixed during resize and explicit fit even when the origin moves",()=>{
 const original=createStyledEditor(source());
 const resized=editStyledEditor(original,{type:"area",bounds:{x:10,y:20,width:90,height:30}});
 expect(styledEditorRequest(resized)).toMatchObject({baselineAnchor:{x:20,y:38}});
 // Fixed dimensions can still occur when fitting adds headroom while an edge
 // changes due to displayed page rotation; intent must override move inference.
 const fit={type:"area" as const,bounds:{...original.present.bounds,x:25,y:40},intent:"fit" as const};
 const fitted=editStyledEditor(original,fit);
 expect(styledEditorRequest(fitted)).toMatchObject({baselineAnchor:{x:20,y:38}});
});
it("keeps absent-anchor legacy selections absent and rejects malformed anchors",()=>{
 const {baselineAnchor:_,...legacy}=source();expect(styledEditorRequest(createStyledEditor(legacy))).not.toHaveProperty("baselineAnchor");
 expect(()=>createStyledEditor({...source(),baselineAnchor:{x:0,y:Number.NaN}})).toThrow();
});
