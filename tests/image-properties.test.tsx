import {useState} from "react";
import {cleanup,fireEvent,render} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ImageProperties} from "../src/components/ImageProperties";
import type {ImageOverlay} from "../src/editor/types";
const image:ImageOverlay={type:"image",id:"photo",assetId:"a".repeat(64),x:40,y:50,width:100,height:60,rotation:0};
afterEach(cleanup);
function setup(initial=image){const changed=vi.fn();function Host(){const [value,setValue]=useState(initial),[locked,setLocked]=useState(true);return <><ImageProperties image={value} pageWidth={300} pageHeight={400} locked={locked} onLockChange={setLocked} onChange={next=>{changed(next);setValue(next);}}/><output data-testid="value">{JSON.stringify(value)}</output></>;}return {...render(<Host/>),changed};}
function enter(view:ReturnType<typeof setup>,label:string,text:string){const field=view.getByRole("spinbutton",{name:label});fireEvent.change(field,{target:{value:text}});fireEvent.keyDown(field,{key:"Enter"});}
it("commits a numeric resize once, preserves aspect ratio, and can unlock dimensions",()=>{
 const view=setup();const width=view.getByRole("spinbutton",{name:"Image width (pt)"});fireEvent.change(width,{target:{value:"150"}});expect(view.changed).not.toHaveBeenCalled();fireEvent.blur(width);expect(view.changed).toHaveBeenCalledOnce();expect(view.changed).toHaveBeenLastCalledWith(expect.objectContaining({width:150,height:90}));
 fireEvent.click(view.getByRole("checkbox",{name:"Lock image proportions"}));enter(view,"Image height (pt)","100");expect(view.changed).toHaveBeenLastCalledWith(expect.objectContaining({width:150,height:100}));
});
it.each(["","0","-1","0.001","1000"])("rejects invalid or out-of-page size %s with an explanation",raw=>{const view=setup();enter(view,"Image width (pt)",raw);expect(view.changed).not.toHaveBeenCalled();expect(view.getByRole("alert")).toBeInTheDocument();});
it("discards Escape and position overflow without losing the original geometry",()=>{const view=setup();const left=view.getByRole("spinbutton",{name:"Image left (pt)"});fireEvent.change(left,{target:{value:"90"}});fireEvent.keyDown(left,{key:"Escape"});fireEvent.blur(left);expect(view.changed).not.toHaveBeenCalled();expect(left).toHaveValue(40);enter(view,"Image top (pt)","380");expect(view.changed).not.toHaveBeenCalled();expect(view.getByRole("alert")).toHaveTextContent("page");});
it("uses visible bounds for rotated numeric positions and quarter-turn buttons",()=>{
 const view=setup({...image,rotation:90,x:100});expect(view.getByRole("spinbutton",{name:"Image left (pt)"})).toHaveValue(40);expect(view.getByRole("spinbutton",{name:"Image width (pt)"})).toHaveValue(60);
 enter(view,"Image left (pt)","60");expect(view.changed).toHaveBeenLastCalledWith(expect.objectContaining({x:120,y:50,rotation:90}));
 fireEvent.click(view.getByRole("button",{name:"Rotate image clockwise"}));expect(view.changed).toHaveBeenLastCalledWith(expect.objectContaining({rotation:180}));
 fireEvent.click(view.getByRole("button",{name:"Rotate image counterclockwise"}));expect(view.changed).toHaveBeenLastCalledWith(expect.objectContaining({rotation:90}));
});
