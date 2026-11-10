import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ExistingTextLayer} from "../src/components/ExistingTextLayer";
import {nativeAdapter} from "../src/editor/adapter";
afterEach(cleanup);
it.each([true,undefined])("carries native form substitution eligibility through selection: %s",async(canSubstitute)=>{
 const onEditText=vi.fn();const run={objectPath:[1,0,0],text:"Café old",fontName:"Subset",fontDisplayName:"DejaVu Serif",fontSize:16,bounds:{x:20,y:30,width:90,height:16},supported:true,isEmbedded:true,canSubstitute};
 const adapter={...nativeAdapter,listOccurrenceFormTextRuns:undefined,listTextRuns:vi.fn().mockResolvedValue({runs:[]}),listFormTextRuns:vi.fn().mockResolvedValue({runs:[run]})};
 render(<svg><ExistingTextLayer adapter={adapter} page={{id:"p",sourceId:"source",pageIndex:0,width:400,height:400,rotation:0,overlays:[]}} onEditText={onEditText}/></svg>);
 fireEvent.click(await screen.findByRole("button",{name:"Edit text: Café old"}));expect(onEditText).toHaveBeenCalledWith(expect.objectContaining({objectPath:[1,0,0],objectIndex:1,isEmbedded:true,fontDisplayName:"DejaVu Serif",canSubstitute:canSubstitute??false}));
});
