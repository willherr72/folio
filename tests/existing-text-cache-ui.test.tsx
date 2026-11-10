import {cleanup,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ExistingTextLayer} from "../src/components/ExistingTextLayer";
import {createDemoAdapter} from "../src/editor/adapter";
import {clearRenderCache} from "../src/components/PageView";

const page={id:"cache-page",sourceId:"cache-ui",pageIndex:0,width:400,height:500,rotation:0 as const,overlays:[]};
const run={objectIndex:1,text:"Original paragraph",fontName:"Helvetica",fontSize:12,bounds:{x:20,y:30,width:100,height:15},supported:true};
afterEach(()=>{cleanup();clearRenderCache([page.sourceId]);});
it("reuses unchanged page inspection when the paragraph workspace unmounts and restores the hit layer",async()=>{
 const adapter={...createDemoAdapter(),listTextRuns:vi.fn().mockResolvedValue({runs:[run]}),listFormTextRuns:vi.fn().mockResolvedValue({runs:[]})};
 const first=render(<svg><ExistingTextLayer adapter={adapter} page={page}/></svg>);
 await screen.findByRole("button",{name:"Edit text: Original paragraph"});first.unmount();
 render(<svg><ExistingTextLayer adapter={adapter} page={page}/></svg>);
 await screen.findByRole("button",{name:"Edit text: Original paragraph"});
 expect(adapter.listTextRuns).toHaveBeenCalledTimes(1);expect(adapter.listFormTextRuns).toHaveBeenCalledTimes(1);
});
it("reinspects released workspace sources even if the same adapter and page ID return",async()=>{
 const adapter={...createDemoAdapter(),listTextRuns:vi.fn().mockResolvedValueOnce({runs:[run]}).mockResolvedValue({runs:[{...run,text:"Reopened source"}]})};
 const first=render(<svg><ExistingTextLayer adapter={adapter} page={page}/></svg>);
 await screen.findByRole("button",{name:"Edit text: Original paragraph"});first.unmount();clearRenderCache([page.sourceId]);
 render(<svg><ExistingTextLayer adapter={adapter} page={page}/></svg>);
 await screen.findByRole("button",{name:"Edit text: Reopened source"});expect(adapter.listTextRuns).toHaveBeenCalledTimes(2);
});
