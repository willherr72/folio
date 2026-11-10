import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {PageOrganizerDialog} from "../src/components/PageOrganizerDialog";
import {createDemoAdapter} from "../src/editor/adapter";
import type {EditorDocument} from "../src/editor/types";
vi.mock("../src/components/PageView",()=>({Thumbnail:({page}:any)=><span>{page.id}</span>}));
const doc=(count=10000):EditorDocument=>({name:"Large.pdf",pages:Array.from({length:count},(_,i)=>({id:`p${i}`,sourceId:"s",pageIndex:i,width:300,height:400,rotation:0,overlays:[]})),selectedPageId:"p0",selectedOverlayId:null});
function setup(document=doc()){const onApply=vi.fn();const view=render(<PageOrganizerDialog document={document} adapter={createDemoAdapter()} onApply={onApply} onClose={vi.fn()} onExtract={vi.fn()}/>);return{...view,onApply};}
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
it("bounds ten-thousand-page tiles while End, Shift+Home and rotation use the full document",()=>{
 const s=setup();expect(screen.getAllByRole("option").length).toBeLessThan(40);
 fireEvent.keyDown(screen.getByRole("option",{name:"Page 1 of 10000"}),{key:"End"});const last=screen.getByRole("option",{name:"Page 10000 of 10000"});expect(last).toHaveFocus();expect(last).toHaveAttribute("aria-posinset","10000");expect(last).toHaveAttribute("aria-setsize","10000");
 fireEvent.keyDown(last,{key:"Home",shiftKey:true});expect(screen.getByText("10000 of 10000 pages selected")).toBeVisible();fireEvent.click(screen.getByRole("button",{name:"Rotate right"}));fireEvent.click(screen.getByRole("button",{name:"Apply arrangement"}));expect(s.onApply.mock.calls[0][0].pages.every((p:any)=>p.rotation===90)).toBe(true);
});
it("keeps the focused row mounted through manual scroll without snapping back",async()=>{
 setup();const first=screen.getByRole("option",{name:"Page 1 of 10000"}),grid=screen.getByRole("listbox");first.focus();grid.scrollTop=100000;fireEvent.scroll(grid);
 await waitFor(()=>expect(screen.getAllByRole("option").some(e=>Number(e.getAttribute("data-page-index"))>400)).toBe(true));expect(first).toHaveFocus();expect(grid.scrollTop).toBe(100000);expect(screen.getAllByRole("option").length).toBeLessThan(40);
});
it("keeps the drag origin mounted when scrolling to a distant drop target",async()=>{
 const s=setup();const first=screen.getByRole("option",{name:"Page 1 of 10000"}),grid=screen.getByRole("listbox"),transfer={setData:vi.fn(),effectAllowed:"",dropEffect:""};fireEvent.dragStart(first,{dataTransfer:transfer});grid.scrollTop=100000;fireEvent.scroll(grid);
 await waitFor(()=>expect(screen.getAllByRole("option").some(e=>Number(e.getAttribute("data-page-index"))>400)).toBe(true));expect(first).toBeInTheDocument();const target=screen.getAllByRole("option").find(e=>Number(e.getAttribute("data-page-index"))>400)!;const before=Number(target.getAttribute("data-page-index"));fireEvent.drop(target,{dataTransfer:transfer});fireEvent.click(screen.getByRole("button",{name:"Apply arrangement"}));expect(screen.queryByText(/unexpected/i)).toBeNull();expect(before).toBeGreaterThan(400);expect(s.onApply.mock.calls[0][0].pages[before-1].id).toBe("p0");expect(s.onApply.mock.calls[0][0].pages[before].id).toBe(`p${before}`);
});
it("uses responsive column count for vertical keys and reveals restored history focus",()=>{
 let callback:()=>void=()=>{};vi.stubGlobal("ResizeObserver",class{constructor(cb:()=>void){callback=cb;}observe(){}disconnect(){}});
 setup(doc(100));const grid=screen.getByRole("listbox");Object.defineProperty(grid,"clientWidth",{configurable:true,value:800});Object.defineProperty(grid,"clientHeight",{configurable:true,value:400});act(()=>callback());
 fireEvent.keyDown(screen.getByRole("option",{name:"Page 1 of 100"}),{key:"ArrowDown"});expect(screen.getByRole("option",{name:"Page 6 of 100"})).toHaveFocus();fireEvent.click(screen.getByRole("button",{name:"Delete selected"}));fireEvent.keyDown(screen.getByRole("button",{name:"Undo page action"}),{key:"z",ctrlKey:true});expect(screen.getByRole("option",{name:"Page 6 of 100"})).toHaveAttribute("aria-selected","true");
});

it("reveals the restored Undo selection without taking focus from its still-enabled toolbar button",async()=>{setup();fireEvent.keyDown(screen.getByRole("option",{name:"Page 1 of 10000"}),{key:"End"});fireEvent.click(screen.getByRole("button",{name:"Rotate right"}));fireEvent.click(screen.getByRole("button",{name:"Rotate right"}));const grid=screen.getByRole("listbox"),undo=screen.getByRole("button",{name:"Undo page action"});undo.focus();grid.scrollTop=0;fireEvent.scroll(grid);await waitFor(()=>expect(screen.getByRole("option",{name:"Page 1 of 10000"})).toBeInTheDocument());fireEvent.click(undo);expect(grid.scrollTop).toBeGreaterThan(1000000);expect(undo).toBeEnabled();expect(undo).toHaveFocus();});

it("retains a middle selected drag origin independently of first selection and focus",async()=>{
 setup(doc(1000));const grid=screen.getByRole("listbox");const scroll=async (index:number)=>{grid.scrollTop=index*198;fireEvent.scroll(grid);await waitFor(()=>expect(screen.getByRole("option",{name:`Page ${index+1} of 1000`})).toBeInTheDocument());};
 await scroll(50);fireEvent.click(screen.getByRole("option",{name:"Page 51 of 1000"}),{ctrlKey:true});await scroll(100);fireEvent.click(screen.getByRole("option",{name:"Page 101 of 1000"}),{ctrlKey:true});await scroll(50);const origin=screen.getByRole("option",{name:"Page 51 of 1000"});fireEvent.dragStart(origin,{dataTransfer:{setData:vi.fn(),effectAllowed:""}});await scroll(300);expect(origin).toBeInTheDocument();fireEvent.dragEnd(origin);expect(screen.queryByRole("option",{name:"Page 51 of 1000"})).toBeNull();
});
it("applies a narrowed-grid anchor after the expanded spacer is committed",async()=>{
 let callback:()=>void=()=>{};vi.stubGlobal("ResizeObserver",class{constructor(cb:()=>void){callback=cb;}observe(){}disconnect(){}});setup(doc(1000));const grid=screen.getByRole("listbox");let width=900,top=0;
 Object.defineProperty(grid,"clientWidth",{configurable:true,get:()=>width});Object.defineProperty(grid,"clientHeight",{configurable:true,value:400});Object.defineProperty(grid,"scrollTop",{configurable:true,get:()=>top,set:value=>{const height=parseFloat((grid.firstElementChild as HTMLElement).style.height);top=Math.max(0,Math.min(value,height-400));}});act(()=>callback());grid.scrollTop=30000;fireEvent.scroll(grid);await waitFor(()=>expect(screen.getAllByRole("option").some(x=>Number(x.getAttribute("data-page-index"))>900)).toBe(true));width=600;act(()=>callback());expect(grid.scrollTop).toBe(44850);
});

it("continues edge scrolling over grid gaps and selected tiles during an owned drag",()=>{setup(doc(100));const grid=screen.getByRole("listbox"),first=screen.getByRole("option",{name:"Page 1 of 100"}),transfer={setData:vi.fn(),effectAllowed:"",dropEffect:""};vi.spyOn(grid,"getBoundingClientRect").mockReturnValue({top:0,bottom:400} as DOMRect);fireEvent.dragStart(first,{dataTransfer:transfer});fireEvent(grid,Object.assign(new Event("dragover",{bubbles:true,cancelable:true}),{dataTransfer:transfer,clientY:390}));expect(grid.scrollTop).toBe(16);fireEvent(first,Object.assign(new Event("dragover",{bubbles:true,cancelable:true}),{dataTransfer:transfer,clientY:390}));expect(grid.scrollTop).toBe(32);fireEvent.dragEnd(first);fireEvent(grid,Object.assign(new Event("dragover",{bubbles:true,cancelable:true}),{dataTransfer:transfer,clientY:390}));expect(grid.scrollTop).toBe(32);});
