import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ExistingTextLayer} from "../src/components/ExistingTextLayer";
import {ExistingTextDialog} from "../src/components/ExistingTextDialog";
import {createDemoAdapter,nativeAdapter} from "../src/editor/adapter";
const ipc=vi.hoisted(()=>vi.fn());vi.mock("@tauri-apps/api/core",()=>({invoke:ipc}));
const page={id:"page",sourceId:"source",pageIndex:1,width:300,height:400,rotation:0 as const,overlays:[]};
const form={objectPath:[2,0,1],text:"Shared words",fontName:"Helvetica",fontSize:12,bounds:{x:30,y:50,width:80,height:15},supported:true};
afterEach(()=>{cleanup();ipc.mockReset();});
it("sends the full occurrence path and expected text through dedicated native commands",async()=>{
 ipc.mockResolvedValue({runs:[form]});await nativeAdapter.listFormTextRuns!("source",1);
 expect(ipc).toHaveBeenLastCalledWith("list_form_text_runs",{sourceId:"source",pageIndex:1});
 await nativeAdapter.replaceFormText!("source",1,[2,0,1],"Shared words","Changed words");
 expect(ipc).toHaveBeenLastCalledWith("replace_form_text",{sourceId:"source",pageIndex:1,objectPath:[2,0,1],expectedText:"Shared words",replacement:"Changed words"});
});
it("keeps separate hit targets and complete paths for identical nested pieces",async()=>{
 const onEditText=vi.fn();const adapter={...createDemoAdapter(),listTextRuns:vi.fn(async()=>({runs:[]})),listFormTextRuns:vi.fn(async()=>({runs:[form,{...form,objectPath:[2,1,1],bounds:{...form.bounds,y:90}}]}))};
 render(<svg><ExistingTextLayer adapter={adapter} page={page} onEditText={onEditText}/></svg>);
 const targets=await screen.findAllByRole("button",{name:"Edit text: Shared words"});expect(targets).toHaveLength(2);
 fireEvent.click(targets[1]);expect(onEditText).toHaveBeenLastCalledWith(expect.objectContaining({objectPath:[2,1,1]}));
 fireEvent.keyDown(targets[0],{key:"Enter"});expect(onEditText).toHaveBeenLastCalledWith(expect.objectContaining({objectPath:[2,0,1]}));
});
it("ignores late form discovery after the source changes and retains top-level results on form errors",async()=>{
 let resolve!:(value:any)=>void;const adapter={...createDemoAdapter(),listTextRuns:vi.fn(async()=>({runs:[{...form,objectPath:undefined,objectIndex:7,text:"Top level"}]})),listFormTextRuns:vi.fn().mockImplementationOnce(()=>new Promise(done=>{resolve=done;})).mockRejectedValue(new Error("Forms cannot be inspected"))};
 const view=render(<svg><ExistingTextLayer adapter={adapter} page={page}/></svg>);
 view.rerender(<svg><ExistingTextLayer adapter={adapter} page={{...page,sourceId:"new"}}/></svg>);
 await screen.findByRole("button",{name:"Edit text: Top level"});
 await act(async()=>resolve({runs:[form]}));expect(screen.queryByRole("button",{name:"Edit text: Shared words"})).not.toBeInTheDocument();
});
it("explains occurrence isolation and excludes grouping and substitution controls",()=>{
 render(<ExistingTextDialog run={{...form,objectIndex:2,canSubstitute:true}} busy={false} error={null} onApply={vi.fn()} onCancel={vi.fn()} onEditTogether={vi.fn()}/>);
 expect(screen.getByText(/Only this occurrence will change/)).toBeVisible();
 expect(screen.queryByRole("button",{name:"Edit together…"})).not.toBeInTheDocument();
 expect(screen.queryByRole("button",{name:"Choose substitute font…"})).not.toBeInTheDocument();
});
