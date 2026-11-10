import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {PageThumbnails} from "../src/components/PageThumbnails";
import {createDemoAdapter} from "../src/editor/adapter";
import type {PagePlan} from "../src/editor/types";
vi.mock("../src/components/PageView",()=>({Thumbnail:({page}:any)=><span>{page.id}</span>}));
const pages:PagePlan[]=Array.from({length:10000},(_,i)=>({id:`p${i}`,sourceId:"s",pageIndex:i,width:300,height:400,rotation:0,overlays:[]}));
const props=()=>({pages,selectedPageId:"p0",adapter:createDemoAdapter(),blocked:false,readingBlocked:false,canArrange:true,draggedPageId:null,dropTarget:null,onSelect:vi.fn(),onDragStart:vi.fn(),onDragOver:vi.fn(),onDrop:vi.fn(),onDragEnd:vi.fn(),onDragLeave:vi.fn()});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
it("mounts bounded buttons and navigates to unmounted pages without trapping Tab",()=>{
 const p=props();render(<PageThumbnails {...p}/>);expect(screen.getAllByRole("button").length).toBeLessThan(20);const first=screen.getByRole("button",{name:"Page 1 of 10000"});fireEvent.keyDown(first,{key:"End"});expect(p.onSelect).toHaveBeenLastCalledWith("p9999");const last=screen.getByRole("button",{name:"Page 10000 of 10000"});expect(last).toHaveFocus();expect(screen.getAllByRole("button").filter(x=>x.tabIndex===0)).toEqual([last]);expect(fireEvent.keyDown(last,{key:"Tab"})).toBe(true);
});
it("reveals selected pages on external navigation but permits independent manual scrolling",async()=>{
 const p=props(),view=render(<PageThumbnails {...p}/>);view.rerender(<PageThumbnails {...p} selectedPageId="p9999"/>);expect(screen.getByRole("button",{name:"Page 10000 of 10000"})).toBeInTheDocument();const host=screen.getByRole("group",{name:"Page thumbnails"});expect(host.scrollTop).toBeGreaterThan(1000000);host.scrollTop=500000;fireEvent.scroll(host);await waitFor(()=>expect(screen.getAllByRole("button").some(x=>Number(x.getAttribute("data-page-index"))>3000&&Number(x.getAttribute("data-page-index"))<4000)).toBe(true));expect(host.scrollTop).toBe(500000);view.rerender(<PageThumbnails {...p} pages={p.pages.map(page=>({...page}))} selectedPageId="p9999"/>);expect(host.scrollTop).toBe(500000);
});
it("retains a drag source and forwards logical targets after scrolling",async()=>{
 const p=props();render(<PageThumbnails {...p} draggedPageId="p0"/>);const first=screen.getByRole("button",{name:"Page 1 of 10000"}),host=screen.getByRole("group",{name:"Page thumbnails"});host.scrollTop=500000;fireEvent.scroll(host);await waitFor(()=>expect(screen.getAllByRole("button").some(x=>Number(x.getAttribute("data-page-index"))>3000)).toBe(true));expect(first).toBeInTheDocument();const target=screen.getAllByRole("button").find(x=>Number(x.getAttribute("data-page-index"))>3000)!;fireEvent.drop(target,{dataTransfer:{}});expect(p.onDrop.mock.calls[0][1]).toBe(target.getAttribute("data-thumbnail-id"));
});
it("blocks keyboard activation and dragging while editing is blocked",()=>{const p=props();render(<PageThumbnails {...p} blocked readingBlocked/>);const first=screen.getByRole("button",{name:"Page 1 of 10000"});expect(first).toBeDisabled();expect(first).toHaveAttribute("draggable","false");fireEvent.keyDown(first,{key:"End"});expect(p.onSelect).not.toHaveBeenCalled();});
