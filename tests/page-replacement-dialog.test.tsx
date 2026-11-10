import {act,cleanup,fireEvent,render,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {PageReplacementDialog} from "../src/components/PageReplacementDialog";
import {createDemoAdapter} from "../src/editor/adapter";
import type {DocumentInfo,EditorDocument,PageContentReplacement,PagePlan} from "../src/editor/types";
const page:PagePlan={id:"p",sourceId:"source",pageIndex:0,width:300,height:400,rotation:90,crop:{x:10,y:20,width:200,height:300},overlays:[]};
const document:EditorDocument={name:"Original.pdf",pages:[page,{...page,id:"copy"}],selectedPageId:"p",selectedOverlayId:null};
const incoming:DocumentInfo={id:"incoming",name:"Incoming.pdf",pages:[{width:500,height:500},{width:200,height:400}]};
const result:PageContentReplacement={document:{id:"candidate",name:"Replaced.pdf",pages:[{width:300,height:400},{width:300,height:400}]},pageIndices:[0,1]};
afterEach(()=>{cleanup();vi.restoreAllMocks();});
function setup(){
 const adapter={...createDemoAdapter(),kind:"native" as const,openPdf:vi.fn(async()=>incoming),closeDocument:vi.fn(async(_id:string)=>{}),replacePageContent:vi.fn(async()=>result),renderPagePlan:vi.fn(async(_page:PagePlan,_width:number)=>"data:image/png;base64,AA")};
 return {adapter,page,document,sourceIds:["source"],onApply:vi.fn(()=>true),onCancel:vi.fn()};
}
async function choose(v:ReturnType<typeof render>){fireEvent.click(v.getByRole("button",{name:"Choose PDF…"}));await v.findByText("Incoming.pdf");}
it("requires rendered preview, keeps workspace transforms, and transfers ownership only on Apply",async()=>{
 const props=setup(),v=render(<PageReplacementDialog {...props}/>);
 expect(v.getByRole("button",{name:"Apply replacement"})).toBeDisabled();await choose(v);
 fireEvent.change(v.getByLabelText("Incoming page"),{target:{value:"2"}});
 fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));
 await waitFor(()=>expect(v.getByRole("button",{name:"Apply replacement"})).toBeEnabled());
 expect(props.adapter.replacePageContent).toHaveBeenCalledWith("source",[0,0],0,"incoming",1);
 expect(props.adapter.renderPagePlan).toHaveBeenCalledWith(expect.objectContaining({sourceId:"candidate",pageIndex:0,crop:page.crop,rotation:90,overlays:page.overlays}),900);
 fireEvent.click(v.getByRole("button",{name:"Apply replacement"}));fireEvent.click(v.getByRole("button",{name:"Apply replacement"}));
 expect(props.onApply).toHaveBeenCalledOnce();expect(props.onApply).toHaveBeenCalledWith(result);v.unmount();
 await waitFor(()=>expect(props.adapter.closeDocument).toHaveBeenCalledWith("incoming"));
 expect(props.adapter.closeDocument).not.toHaveBeenCalledWith("candidate");expect(props.adapter.closeDocument).not.toHaveBeenCalledWith("source");
});
it("changing incoming page discards the candidate and requires another preview",async()=>{
 const props=setup(),v=render(<PageReplacementDialog {...props}/>);await choose(v);
 fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));await v.findByAltText("Replacement page preview");
 fireEvent.change(v.getByLabelText("Incoming page"),{target:{value:"2"}});
 expect(v.getByRole("button",{name:"Apply replacement"})).toBeDisabled();await waitFor(()=>expect(props.adapter.closeDocument).toHaveBeenCalledWith("candidate"));
 for(const value of ["","0","3","1.5"]){fireEvent.change(v.getByLabelText("Incoming page"),{target:{value}});expect(v.getByRole("button",{name:"Preview replacement"})).toBeDisabled();}
});
it("cancel and late native completion release the candidate without applying",async()=>{
 const props=setup();let done!:(value:PageContentReplacement)=>void;
 props.adapter.replacePageContent.mockImplementation(()=>new Promise(resolve=>{done=resolve;}));
 const v=render(<PageReplacementDialog {...props}/>);await choose(v);fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));
 fireEvent.click(v.getByRole("button",{name:"Cancel"}));expect(props.onCancel).toHaveBeenCalledOnce();v.unmount();
 await act(async()=>{});expect(props.adapter.closeDocument).not.toHaveBeenCalledWith("incoming");
 await act(async()=>done(result));await waitFor(()=>expect(props.adapter.closeDocument).toHaveBeenCalledWith("candidate"));await waitFor(()=>expect(props.adapter.closeDocument).toHaveBeenCalledWith("incoming"));expect(props.onApply).not.toHaveBeenCalled();
});
it("a native or render failure cannot authorize Apply",async()=>{
 const props=setup();props.adapter.replacePageContent.mockRejectedValueOnce(new Error("Unsupported incoming content"));
 const v=render(<PageReplacementDialog {...props}/>);await choose(v);fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));
 expect(await v.findByRole("alert")).toHaveTextContent("Unsupported incoming content");
 props.adapter.renderPagePlan.mockImplementation(async plan=>{if(plan.sourceId==="candidate")throw new Error("Preview failed");return "data:image/png;base64,AA";});
 fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));expect(await v.findByText("Preview failed")).toBeTruthy();
 expect(v.getByRole("button",{name:"Apply replacement"})).toBeDisabled();v.unmount();await waitFor(()=>expect(props.adapter.closeDocument).toHaveBeenCalledWith("candidate"));
});
it("closes a PDF returned after the picker was cancelled by unmount",async()=>{
 const props=setup();let done!:(value:DocumentInfo)=>void;props.adapter.openPdf.mockImplementation(()=>new Promise(resolve=>{done=resolve;}));
 const v=render(<PageReplacementDialog {...props}/>);fireEvent.click(v.getByRole("button",{name:"Choose PDF…"}));v.unmount();
 await act(async()=>done(incoming));await waitFor(()=>expect(props.adapter.closeDocument).toHaveBeenCalledWith("incoming"));expect(props.onApply).not.toHaveBeenCalled();
});
it("changing the draft while native work runs cannot adopt its late result",async()=>{
 const props=setup();let done!:(value:PageContentReplacement)=>void;
 props.adapter.replacePageContent.mockImplementation(()=>new Promise(resolve=>{done=resolve;}));
 const v=render(<PageReplacementDialog {...props}/>);await choose(v);fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));
 fireEvent.change(v.getByLabelText("Incoming page"),{target:{value:"2"}});await act(async()=>done(result));
 await waitFor(()=>expect(props.adapter.closeDocument).toHaveBeenCalledWith("candidate"));
 expect(v.getByRole("button",{name:"Apply replacement"})).toBeDisabled();expect(v.queryByAltText("Replacement page preview")).toBeNull();
});
it("a rejected Apply keeps ownership until the dialog closes",async()=>{
 const props=setup();props.onApply.mockReturnValue(false);
 const v=render(<PageReplacementDialog {...props}/>);await choose(v);fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));
 await waitFor(()=>expect(v.getByRole("button",{name:"Apply replacement"})).toBeEnabled());fireEvent.click(v.getByRole("button",{name:"Apply replacement"}));
 expect(await v.findByRole("alert")).toHaveTextContent("document changed");expect(props.adapter.closeDocument).not.toHaveBeenCalledWith("candidate");
 v.unmount();await waitFor(()=>expect(props.adapter.closeDocument).toHaveBeenCalledWith("candidate"));
});
it.each(["another-tab","history-only"])("rejects borrowed candidate %s without rendering, adopting, or closing it",async borrowed=>{
 const props=setup();props.sourceIds.push(borrowed);
 props.adapter.replacePageContent.mockResolvedValue({...result,document:{...result.document,id:borrowed}});
 const v=render(<PageReplacementDialog {...props}/>);await choose(v);fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));
 expect(await v.findByRole("alert")).toHaveTextContent("new source");
 expect(v.getByRole("button",{name:"Apply replacement"})).toBeDisabled();
 expect(props.adapter.renderPagePlan).not.toHaveBeenCalledWith(expect.objectContaining({sourceId:borrowed}),expect.anything());
 v.unmount();await waitFor(()=>expect(props.adapter.closeDocument).toHaveBeenCalledWith("incoming"));
 expect(props.adapter.closeDocument).not.toHaveBeenCalledWith(borrowed);expect(props.onApply).not.toHaveBeenCalled();
});
it("a late render of an invalidated candidate never enables Apply",async()=>{
 const props=setup();let done!:(url:string)=>void;
 props.adapter.renderPagePlan.mockImplementation(plan=>plan.sourceId==="candidate"?new Promise(resolve=>{done=resolve;}):Promise.resolve("data:image/png;base64,AA"));
 const v=render(<PageReplacementDialog {...props}/>);await choose(v);fireEvent.click(v.getByRole("button",{name:"Preview replacement"}));
 await waitFor(()=>expect(done).toBeTruthy());expect(v.getByRole("button",{name:"Apply replacement"})).toBeDisabled();
 fireEvent.change(v.getByLabelText("Incoming page"),{target:{value:"2"}});await act(async()=>done("data:image/png;base64,BB"));
 expect(v.getByRole("button",{name:"Apply replacement"})).toBeDisabled();expect(v.queryByAltText("Replacement page preview")).toBeNull();
 await waitFor(()=>expect(props.adapter.closeDocument.mock.calls.filter(([id])=>id==="candidate")).toHaveLength(1));
});
