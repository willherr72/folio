import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ExtractPagesDialog} from "../src/components/ExtractPagesDialog";
import type {PagePlan} from "../src/editor/types";
afterEach(cleanup);
const pages:PagePlan[]=Array.from({length:5},(_,i)=>({id:`p${i}`,sourceId:"s",pageIndex:i,width:300,height:400,rotation:0,overlays:[]}));
function setup(onExport=vi.fn().mockResolvedValue(false)){
 const onClose=vi.fn();render(<ExtractPagesDialog pages={pages} currentPageId="p2" onClose={onClose} onExport={onExport}/>);return {onExport,onClose};
}
it("defaults to current page and extracts unique range pages in document order",async()=>{
 const {onExport}=setup();
 fireEvent.click(screen.getByRole("button",{name:"Save extracted PDF…"}));await waitFor(()=>expect(onExport).toHaveBeenCalledWith([pages[2]]));
 await waitFor(()=>expect(screen.getByRole("button",{name:"Save extracted PDF…"})).toBeEnabled());
 fireEvent.change(screen.getByRole("combobox",{name:"Pages"}),{target:{value:"range"}});
 const input=screen.getByRole("textbox",{name:"Page range"});
 fireEvent.change(input,{target:{value:"5, 2-3, 2"}});
 expect(screen.getByText("3 pages selected.")).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Save extracted PDF…"}));await waitFor(()=>expect(onExport).toHaveBeenLastCalledWith([pages[1],pages[2],pages[4]]));
 expect(input).toHaveValue("5, 2-3, 2");
});
it("blocks invalid ranges and retains a failed export draft for retry",async()=>{
 const {onExport}=setup(vi.fn().mockRejectedValue(new Error("Disk full")));
 fireEvent.change(screen.getByRole("combobox",{name:"Pages"}),{target:{value:"range"}});
 const input=screen.getByRole("textbox",{name:"Page range"});
 fireEvent.change(input,{target:{value:"6"}});expect(screen.getByRole("button",{name:"Save extracted PDF…"})).toBeDisabled();
 fireEvent.change(input,{target:{value:"1-2"}});fireEvent.click(screen.getByRole("button",{name:"Save extracted PDF…"}));
 expect(await screen.findByRole("alert")).toHaveTextContent("Disk full");expect(input).toHaveValue("1-2");
 expect(onExport).toHaveBeenCalledTimes(1);expect(screen.getByRole("button",{name:"Save extracted PDF…"})).toBeEnabled();
});
it("prevents duplicate submission and modal dismissal while saving",async()=>{
 let done!:(value:boolean)=>void;const {onClose,onExport}=setup(vi.fn(()=>new Promise<boolean>(resolve=>done=resolve)));
 const save=screen.getByRole("button",{name:"Save extracted PDF…"});fireEvent.click(save);fireEvent.click(save);
 expect(onExport).toHaveBeenCalledTimes(1);expect(screen.getByRole("button",{name:"Cancel"})).toBeDisabled();
 fireEvent.keyDown(screen.getByRole("dialog"),{key:"Escape"});expect(onClose).not.toHaveBeenCalled();
 await act(async()=>done(true));expect(onClose).toHaveBeenCalledTimes(1);
});
