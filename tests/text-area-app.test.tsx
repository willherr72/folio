import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { App } from "../src/App";
import { nativeAdapter } from "../src/editor/adapter";
import { fontApi, flushFontReleases } from "../src/editor/custom-fonts";
import { clearShapedTextCache, shapedTextApi } from "../src/editor/shaped-text";

const id="c".repeat(64), info={id,name:"Area Serif",weight:400,italic:false,coverage:[[32,126]] as [number,number][]};
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(props:any)=><>
 <button onClick={()=>props.onAddTextArea?.(props.pages[0].id,{x:40,y:70,width:180,height:120})}>Draw area</button>
 <button onClick={()=>props.onResizeTextArea?.(props.pages[0].id,props.pages[0].overlays[0]?.id,{x:40,y:70,width:260,height:160})}>Resize area</button>
 <output data-testid="page-data">{JSON.stringify(props.pages)}</output>
</>}));
beforeEach(()=>{
 localStorage.clear();vi.stubGlobal("FontFace",class{load(){return Promise.resolve(this);}});
 Object.defineProperty(document,"fonts",{configurable:true,value:{add:vi.fn(),delete:vi.fn()}});
 vi.spyOn(fontApi,"listInstalled").mockResolvedValue([{id:"installed",name:info.name,supported:true}]);
 vi.spyOn(fontApi,"loadInstalled").mockResolvedValue(info);vi.spyOn(fontApi,"info").mockResolvedValue(info);
 vi.spyOn(fontApi,"bytes").mockResolvedValue(new Uint8Array([1,2]).buffer);vi.spyOn(fontApi,"release").mockResolvedValue();
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"areas",name:"Areas.pdf",pages:[{width:500,height:600}]});
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");vi.spyOn(nativeAdapter,"exportPdf").mockResolvedValue("Saved.pdf");vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 vi.spyOn(shapedTextApi,"prepare").mockImplementation(async overlay=>({preview:{version:1,fontId:id,text:overlay.text,width:overlay.area?.width??180,height:overlay.area?.height??120,rotation:0,color:[0,0,0],outlines:[],glyphs:[]},origin:[0,102],bounds:{x:0,y:0,width:overlay.area?.width??180,height:overlay.area?.height??120},characters:[]}));
});
afterEach(async()=>{cleanup();clearShapedTextCache();await act(async()=>{await Promise.resolve();await flushFontReleases();});vi.restoreAllMocks();vi.unstubAllGlobals();});
const overlays=()=>JSON.parse(screen.getByTestId("page-data").textContent!)[0].overlays;
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Areas.pdf"});fireEvent.click(screen.getByRole("button",{name:"Text area"}));}
async function choose(){fireEvent.click(await screen.findByRole("button",{name:info.name}));await waitFor(()=>expect(screen.getByRole("button",{name:"Apply font"})).toBeEnabled());fireEvent.click(screen.getByRole("button",{name:"Apply font"}));await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());}
it("creates only after choosing a font, then resizes in one undo step and saves",async()=>{
 await open();fireEvent.click(screen.getByRole("button",{name:"Draw area"}));expect(overlays()).toEqual([]);await choose();
 expect(overlays()).toHaveLength(1);expect(overlays()[0]).toMatchObject({type:"text",x:40,y:70,text:"",fontId:id,fontSize:18,shaping:{version:1,direction:"ltr",ligatures:true},area:{version:1,width:180,height:120}});
 expect(screen.getByRole("textbox",{name:"Content"})).toHaveFocus();
 fireEvent.change(screen.getByRole("textbox",{name:"Content"}),{target:{value:"First paragraph\nSecond"}});
 fireEvent.click(screen.getByRole("button",{name:"Resize area"}));expect(overlays()[0].area.width).toBe(260);
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(overlays()[0].area.width).toBe(180);expect(overlays()[0].text).toBe("First paragraph\nSecond");
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(overlays()[0].area.width).toBe(260);
 fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalledOnce());
 expect(vi.mocked(nativeAdapter.exportPdf).mock.calls[0][0][0].overlays[0]).toMatchObject({fontId:id,area:{width:260,height:160},text:"First paragraph\nSecond"});
 expect(fontApi.release).not.toHaveBeenCalled();
});
it("cancels font selection without adding an overlay or undo history",async()=>{
 await open();fireEvent.click(screen.getByRole("button",{name:"Draw area"}));await screen.findByRole("dialog");fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
 expect(overlays()).toEqual([]);expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
});
it("offers keyboard creation and preserves the created area through undo/redo",async()=>{
 await open();fireEvent.click(screen.getByRole("button",{name:"Add text area"}));await choose();
 expect(overlays()[0]).toMatchObject({x:24,y:24,area:{width:240,height:144}});
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(overlays()).toEqual([]);expect(fontApi.release).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"Redo"}));expect(overlays()[0].fontId).toBe(id);
});

it("creates a keyboard text area within a crop away from the original origin",async()=>{
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"renderPagePlan").mockResolvedValue("data:image/png;base64,AA");
 await open();fireEvent.click(screen.getByRole("button",{name:"Select"}));fireEvent.click(screen.getByRole("button",{name:"Crop page…"}));
 for(const [side,value] of [["Left",100],["Top",400],["Right",200],["Bottom",100]])fireEvent.change(screen.getByLabelText(`${side} margin (pt)`),{target:{value:String(value)}});
 fireEvent.click(screen.getByRole("button",{name:"Preview crop"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Apply crop"})).toBeEnabled());
 fireEvent.click(screen.getByRole("button",{name:"Apply crop"}));await waitFor(()=>expect(screen.queryByRole("dialog")).toBeNull());
 fireEvent.click(screen.getByRole("button",{name:"Text area"}));fireEvent.click(screen.getByRole("button",{name:"Add text area"}));await choose();
 expect(overlays()[0]).toMatchObject({x:124,y:424,area:{width:152,height:52}});
});
