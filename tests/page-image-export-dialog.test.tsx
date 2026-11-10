import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ExportImagesDialog} from "../src/components/ExportImagesDialog";
import {imageExportApi} from "../src/editor/page-image-export";
import type {PagePlan} from "../src/editor/types";
const pages:PagePlan[]=[0,1].map(i=>({id:`p${i}`,sourceId:"s",pageIndex:i,width:300,height:400,rotation:0,overlays:[]}));
afterEach(()=>{cleanup();vi.restoreAllMocks();});
it.each(["", "0"])("exports PNG with an invalid hidden JPEG quality draft %j",async draft=>{
 vi.spyOn(imageExportApi,"register").mockResolvedValue("job");vi.spyOn(imageExportApi,"cancel").mockResolvedValue(false);
 const start=vi.spyOn(imageExportApi,"start").mockResolvedValue({folder:"out",count:1});
 render(<ExportImagesDialog pages={pages} currentPageId="p0" onClose={vi.fn()} onComplete={vi.fn()} onBusyChange={vi.fn()}/>);
 fireEvent.change(screen.getByLabelText("Format"),{target:{value:"jpeg"}});
 fireEvent.change(screen.getByLabelText("JPEG quality"),{target:{value:draft}});
 expect(screen.getByRole("button",{name:"Choose folder and export…"})).toBeDisabled();
 fireEvent.change(screen.getByLabelText("Format"),{target:{value:"png"}});
 expect(screen.getByRole("button",{name:"Choose folder and export…"})).toBeEnabled();
 fireEvent.click(screen.getByRole("button",{name:"Choose folder and export…"}));
 await waitFor(()=>expect(start).toHaveBeenCalledOnce());
 expect(start.mock.calls[0][1].options).toEqual({format:"png",dpi:150,jpegQuality:90});
});
it("exports selected original page numbers and reports success once",async()=>{
 vi.spyOn(imageExportApi,"register").mockResolvedValue("job");vi.spyOn(imageExportApi,"cancel").mockResolvedValue(false);const start=vi.spyOn(imageExportApi,"start").mockResolvedValue({folder:"out",count:1});const complete=vi.fn();
 render(<ExportImagesDialog pages={pages} currentPageId="p1" onClose={vi.fn()} onComplete={complete} onBusyChange={vi.fn()}/>);
 expect(screen.getByText(/625 × 833/)).toBeTruthy();fireEvent.change(screen.getByLabelText("Format"),{target:{value:"jpeg"}});fireEvent.change(screen.getByLabelText("JPEG quality"),{target:{value:"95"}});
 fireEvent.click(screen.getByRole("button",{name:"Choose folder and export…"}));fireEvent.click(screen.getByRole("button",{name:"Choose folder and export…"}));
 await waitFor(()=>expect(complete).toHaveBeenCalledOnce());expect(start).toHaveBeenCalledOnce();expect(start.mock.calls[0][1]).toMatchObject({pageNumbers:[2],options:{format:"jpeg",dpi:150,jpegQuality:95}});
});
it("keeps the modal open until cancellation completes and ignores repeat starts",async()=>{
 vi.spyOn(imageExportApi,"register").mockResolvedValue("job");const cancel=vi.spyOn(imageExportApi,"cancel").mockResolvedValue(true);let finish!:(value:null)=>void;
 vi.spyOn(imageExportApi,"start").mockImplementation(()=>new Promise(r=>{finish=r;}));const close=vi.fn(),busy=vi.fn();
 render(<ExportImagesDialog pages={pages} currentPageId="p0" onClose={close} onComplete={vi.fn()} onBusyChange={busy}/>);
 fireEvent.click(screen.getByRole("button",{name:"Choose folder and export…"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));
 fireEvent.click(screen.getByRole("button",{name:"Cancel export"}));expect(close).not.toHaveBeenCalled();await waitFor(()=>expect(cancel).toHaveBeenCalledWith("job"));expect(screen.getByText(/Cancelling/)).toBeTruthy();finish(null);
 await waitFor(()=>expect(screen.getByRole("button",{name:"Choose folder and export…"})).toBeEnabled());expect(busy).toHaveBeenLastCalledWith(false);
});
it("disables output for invalid ranges or resolution limits",()=>{
 render(<ExportImagesDialog pages={pages} currentPageId="p0" onClose={vi.fn()} onComplete={vi.fn()} onBusyChange={vi.fn()}/>);
 fireEvent.change(screen.getByLabelText("Pages"),{target:{value:"range"}});fireEvent.change(screen.getByLabelText("Page range"),{target:{value:"4"}});expect(screen.getByRole("button",{name:"Choose folder and export…"})).toBeDisabled();expect(screen.getByRole("alert")).toHaveTextContent(/between 1 and 2/);
});
