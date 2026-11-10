import {cleanup,fireEvent,render,screen,waitFor,act} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {SplitPdfDialog} from "../src/components/SplitPdfDialog";
import {splitPdfApi} from "../src/editor/pdf-split";
import type {PagePlan} from "../src/editor/types";
const pages:PagePlan[]=[0,1,2].map(i=>({id:`p${i}`,sourceId:"s",pageIndex:i,width:300,height:400,rotation:0,overlays:[]}));
const props={pages,onClose:vi.fn(),onComplete:vi.fn(),onBusyChange:vi.fn()};
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.clearAllMocks();});
it("previews contiguous parts and blocks invalid final-page boundaries",()=>{render(<SplitPdfDialog {...props}/>);expect(screen.getByText("part-0003.pdf")).toBeVisible();fireEvent.change(screen.getByLabelText("Split method"),{target:{value:"after"}});fireEvent.change(screen.getByLabelText("Split after pages"),{target:{value:"3"}});expect(screen.getByRole("button",{name:"Choose folder and split…"})).toBeDisabled();fireEvent.change(screen.getByLabelText("Split after pages"),{target:{value:"1"}});expect(screen.getByText("2–3")).toBeVisible();expect(screen.queryByText("part-0003.pdf")).toBeNull();});
it("does not silently discard unreadable bookmarks",()=>{render(<SplitPdfDialog {...props} bookmarkError="unreadable"/>);expect(screen.getByRole("alert")).toHaveTextContent(/Bookmarks panel/);expect(screen.getByRole("button",{name:"Choose folder and split…"})).toBeDisabled();});
it("keeps the operation busy until native cancellation settles and supports retry",async()=>{
 vi.spyOn(splitPdfApi,"register").mockResolvedValue("job");const cancel=vi.spyOn(splitPdfApi,"cancel").mockResolvedValue(true);let finish!:(v:null)=>void;vi.spyOn(splitPdfApi,"start").mockImplementation(()=>new Promise(r=>finish=r));render(<SplitPdfDialog {...props}/>);
 fireEvent.click(screen.getByRole("button",{name:"Choose folder and split…"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));expect(screen.getByLabelText("Split method")).toBeDisabled();fireEvent.click(screen.getByRole("button",{name:"Cancel export"}));expect(props.onClose).not.toHaveBeenCalled();await waitFor(()=>expect(cancel).toHaveBeenCalledWith("job"));await act(async()=>finish(null));expect(props.onBusyChange).toHaveBeenLastCalledWith(false);expect(screen.getByText(/Export cancelled/)).toBeVisible();expect(screen.getByRole("button",{name:"Choose folder and split…"})).toBeEnabled();
});
it("reports errors and releases busy ownership on unmount with a pending result",async()=>{
 vi.spyOn(splitPdfApi,"register").mockResolvedValue("job");const cancel=vi.spyOn(splitPdfApi,"cancel").mockResolvedValue(true);let reject!:(e:Error)=>void;vi.spyOn(splitPdfApi,"start").mockImplementation(()=>new Promise((_r,j)=>reject=j));const view=render(<SplitPdfDialog {...props}/>);fireEvent.click(screen.getByRole("button",{name:"Choose folder and split…"}));await waitFor(()=>expect(reject).toBeTypeOf("function"));view.unmount();await act(async()=>reject(Error("failed")));expect(cancel).toHaveBeenCalledWith("job");expect(props.onBusyChange).toHaveBeenLastCalledWith(false);expect(props.onComplete).not.toHaveBeenCalled();
});
