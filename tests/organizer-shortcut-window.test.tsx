import {cleanup,fireEvent,render,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {PageOrganizerDialog} from "../src/components/PageOrganizerDialog";
import {createDemoAdapter} from "../src/editor/adapter";
import type {EditorDocument} from "../src/editor/types";
vi.mock("../src/components/PageView",()=>({Thumbnail:()=>null}));
afterEach(cleanup);

it("reveals the distant selected page when a shortcut exhausts history, then handles Escape",async()=>{
 const document:EditorDocument={name:"Large.pdf",pages:Array.from({length:10000},(_,i)=>({id:`p${i}`,sourceId:"s",pageIndex:i,width:300,height:400,rotation:0,overlays:[]})),selectedPageId:"p0",selectedOverlayId:null};
 const onClose=vi.fn(),v=render(<PageOrganizerDialog document={document} adapter={createDemoAdapter()} onApply={vi.fn()} onClose={onClose} onExtract={vi.fn()}/>);
 fireEvent.keyDown(v.getByRole("option",{name:"Page 1 of 10000"}),{key:"End"});fireEvent.click(v.getByRole("button",{name:"Rotate right"}));
 const apply=v.getByRole("button",{name:"Apply arrangement"}),grid=v.getByRole("listbox");apply.focus();grid.scrollTop=0;fireEvent.scroll(grid);
 await waitFor(()=>expect(v.getByRole("option",{name:"Page 1 of 10000"})).toBeInTheDocument());
 fireEvent.keyDown(apply,{key:"z",ctrlKey:true});expect(apply).toBeDisabled();
 expect(grid.scrollTop).toBeGreaterThan(1000000);expect(v.getAllByRole("option").length).toBeLessThan(40);
 expect(v.getByRole("option",{name:"Page 10000 of 10000"})).toHaveFocus();
 fireEvent.keyDown(window.document.activeElement!,{key:"Escape"});expect(onClose).toHaveBeenCalledOnce();
});
