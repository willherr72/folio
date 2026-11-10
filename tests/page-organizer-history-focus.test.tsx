import {cleanup,fireEvent,render} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {PageOrganizerDialog} from "../src/components/PageOrganizerDialog";
import {createDemoAdapter} from "../src/editor/adapter";
import type {EditorDocument} from "../src/editor/types";
vi.mock("../src/components/PageView",()=>({Thumbnail:()=>null}));
const document:EditorDocument={name:"Pages.pdf",pages:[0,1].map(i=>({id:`p${i}`,sourceId:"s",pageIndex:i,width:300,height:400,rotation:0 as const,overlays:[]})),selectedPageId:"p0",selectedOverlayId:null};
function setup(){const onClose=vi.fn();return {...render(<PageOrganizerDialog document={document} adapter={createDemoAdapter()} onClose={onClose} onApply={vi.fn()} onExtract={vi.fn()}/>),onClose};}
afterEach(cleanup);

it.each(["Undo","Redo"])("keeps Escape reachable when the last %s disables its button",action=>{
 const v=setup();fireEvent.click(v.getByRole("button",{name:"Rotate right"}));
 if(action==="Redo")fireEvent.click(v.getByRole("button",{name:"Undo page action"}));
 const button=v.getByRole("button",{name:`${action} page action`});button.focus();fireEvent.click(button);
 expect(button).toBeDisabled();expect(v.getByRole("option",{name:"Page 1 of 2"})).toHaveFocus();
 fireEvent.keyDown(window.document.activeElement!,{key:"Escape"});expect(v.onClose).toHaveBeenCalledOnce();
});

it("keeps an available Undo button focused for repeated keyboard activation",()=>{
 const v=setup(),rotate=v.getByRole("button",{name:"Rotate right"});fireEvent.click(rotate);fireEvent.click(rotate);
 const undo=v.getByRole("button",{name:"Undo page action"});undo.focus();fireEvent.click(undo);
 expect(undo).toBeEnabled();expect(undo).toHaveFocus();
 fireEvent.click(undo);expect(undo).toBeDisabled();expect(v.getByRole("option",{name:"Page 1 of 2"})).toHaveFocus();
});

it.each([
 ["Apply arrangement","z",false],
 ["Undo page action","z",false],
 ["Redo page action","y",false],
 ["Redo page action","z",true],
] as const)("restores page focus after a history shortcut disables %s (%s, shift=%s)",(name,key,shiftKey)=>{
 const v=setup();fireEvent.click(v.getByRole("button",{name:"Rotate right"}));
 if(name==="Redo page action")fireEvent.click(v.getByRole("button",{name:"Undo page action"}));
 const control=v.getByRole("button",{name});control.focus();fireEvent.keyDown(control,{key,ctrlKey:true,shiftKey});
 expect(control).toBeDisabled();expect(v.getByRole("option",{name:"Page 1 of 2"})).toHaveFocus();
 fireEvent.keyDown(window.document.activeElement!,{key:"Escape"});expect(v.onClose).toHaveBeenCalledOnce();
});

it("preserves an available footer control during history shortcuts",()=>{
 const v=setup();fireEvent.click(v.getByRole("button",{name:"Rotate right"}));
 const cancel=v.getByRole("button",{name:"Cancel"});cancel.focus();fireEvent.keyDown(cancel,{key:"z",ctrlKey:true});
 expect(cancel).toHaveFocus();expect(v.getByRole("button",{name:"Apply arrangement"})).toBeDisabled();
 fireEvent.keyDown(cancel,{key:"y",ctrlKey:true});expect(cancel).toHaveFocus();expect(v.getByRole("button",{name:"Apply arrangement"})).toBeEnabled();
});

it("leaves history shortcuts in the page-number input to native text editing",()=>{
 const v=setup();fireEvent.click(v.getByRole("button",{name:"Rotate right"}));
 const input=v.getByRole("spinbutton",{name:"Move before page"});input.focus();fireEvent.change(input,{target:{value:"2"}});
 fireEvent.keyDown(input,{key:"z",ctrlKey:true});expect(input).toHaveFocus();expect(v.getByRole("button",{name:"Apply arrangement"})).toBeEnabled();
});
