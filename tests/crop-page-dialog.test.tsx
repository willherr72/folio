import {cleanup,fireEvent,render,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {CropPageDialog} from "../src/components/CropPageDialog";
import {createDemoAdapter} from "../src/editor/adapter";
import type {PagePlan} from "../src/editor/types";
const page:PagePlan={id:"p",sourceId:"s",pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
function setup(p=page){const onApply=vi.fn(),onCancel=vi.fn(),renderPagePlan=vi.fn(async(_page:PagePlan,_width:number)=>"data:image/png;base64,AA");const adapter={...createDemoAdapter(),kind:"native" as const,renderPagePlan};return {onApply,onCancel,adapter,page:p};}
it("requires an exact current preview and applies one crop",async()=>{
  const p=setup(),v=render(<CropPageDialog {...p}/>);
  expect(v.getByText(/does not remove/i)).toBeTruthy();expect(v.getByRole("button",{name:"Apply crop"})).toBeDisabled();
  fireEvent.change(v.getByLabelText("Left margin (pt)"),{target:{value:"20"}});
  fireEvent.change(v.getByLabelText("Top margin (pt)"),{target:{value:"30"}});
  fireEvent.click(v.getByRole("button",{name:"Preview crop"}));
  await waitFor(()=>expect(v.getByRole("button",{name:"Apply crop"})).toBeEnabled());
  fireEvent.click(v.getByRole("button",{name:"Apply crop"}));fireEvent.click(v.getByRole("button",{name:"Apply crop"}));
  expect(p.onApply).toHaveBeenCalledTimes(1);expect(p.onApply).toHaveBeenCalledWith({x:20,y:30,width:280,height:370});
});
it("rejects blank and inverted margins and resets invalid fields",async()=>{
  const p=setup(),v=render(<CropPageDialog {...p}/>);const left=v.getByLabelText("Left margin (pt)");
  fireEvent.change(left,{target:{value:""}});expect(v.getByRole("button",{name:"Preview crop"})).toBeDisabled();
  fireEvent.change(left,{target:{value:"300"}});expect(v.getByRole("button",{name:"Preview crop"})).toBeDisabled();
  fireEvent.click(v.getByRole("button",{name:"Reset crop"}));expect(left).toHaveValue(0);
  expect(v.getByRole("button",{name:"Preview crop"})).toBeEnabled();fireEvent.click(v.getByRole("button",{name:"Cancel"}));
  expect(p.onCancel).toHaveBeenCalledOnce();expect(p.onApply).not.toHaveBeenCalled();
});
it("a late preview cannot authorize a changed draft",async()=>{
  const p=setup();let resolve!:(url:string)=>void;
  p.adapter.renderPagePlan.mockImplementation(async plan=>plan.crop?new Promise<string>(r=>{resolve=r;}):"data:image/png;base64,AA");
  const v=render(<CropPageDialog {...p}/>);fireEvent.change(v.getByLabelText("Left margin (pt)"),{target:{value:"20"}});
  fireEvent.click(v.getByRole("button",{name:"Preview crop"}));await waitFor(()=>expect(resolve).toBeTruthy());
  fireEvent.change(v.getByLabelText("Left margin (pt)"),{target:{value:"25"}});resolve("data:image/png;base64,BB");
  await waitFor(()=>expect(v.getByRole("button",{name:"Apply crop"})).toBeDisabled());expect(p.onApply).not.toHaveBeenCalled();
});
it("reports native preview failure and keeps apply disabled",async()=>{
  const p=setup();p.adapter.renderPagePlan.mockImplementation(async plan=>{if(plan.crop)throw new Error("Crop could not render");return "data:image/png;base64,AA";});
  const v=render(<CropPageDialog {...p}/>);fireEvent.change(v.getByLabelText("Left margin (pt)"),{target:{value:"20"}});fireEvent.click(v.getByRole("button",{name:"Preview crop"}));
  expect(await v.findByText("Crop could not render")).toBeTruthy();expect(v.getByRole("button",{name:"Apply crop"})).toBeDisabled();
});
