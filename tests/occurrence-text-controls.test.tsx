import {cleanup,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ExistingTextLayer} from "../src/components/ExistingTextLayer";
import {createDemoAdapter} from "../src/editor/adapter";
const page={id:"page",sourceId:"source",pageIndex:2,width:300,height:400,rotation:0 as const,overlays:[]};
const run={objectIndex:0,text:"Shared field document text",fontName:"Helvetica",fontSize:12,bounds:{x:30,y:40,width:100,height:15},supported:true};
afterEach(cleanup);
it("uses complete-source inspection only when its apply capability is available",async()=>{
 const adapter={...createDemoAdapter(),listTextRuns:vi.fn().mockRejectedValue(new Error("shared fields")),listOccurrenceTextRuns:vi.fn().mockResolvedValue({runs:[run]}),replaceTextOccurrence:vi.fn()};
 const view=render(<svg><ExistingTextLayer adapter={adapter} page={page}/></svg>);
 await screen.findByRole("button",{name:`Edit text: ${run.text}`});
 expect(adapter.listOccurrenceTextRuns).toHaveBeenCalledWith("source",2);expect(adapter.listTextRuns).not.toHaveBeenCalled();
 view.rerender(<svg><ExistingTextLayer adapter={{...adapter,replaceTextOccurrence:undefined}} page={page}/></svg>);
 await waitFor(()=>expect(adapter.listTextRuns).toHaveBeenCalled());await screen.findByText("shared fields");
});
