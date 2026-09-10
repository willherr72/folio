import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {useState} from "react";
import {afterEach,expect,it} from "vitest";
import {ReviewList} from "../src/components/ReviewList";
import type {PagePlan} from "../src/editor/types";
afterEach(cleanup);
it("exposes the selected review item across duplicated annotation IDs on different pages",()=>{
 const pages:PagePlan[]=["first","duplicate"].map(id=>({id,sourceId:"source",pageIndex:0,width:200,height:300,rotation:0,overlays:[{type:"comment",id:"note",x:20,y:30,text:"Check this",color:"#FFE066"}]}));
 function Example(){const [selected,setSelected]=useState("first");return <ReviewList pages={pages} selectedPageId={selected} selectedOverlayId="note" disabled={false} onSelect={setSelected}/>;}
 render(<Example/>);
 expect(screen.getByRole("button",{name:"Page 1: Check this",pressed:true})).toBeInTheDocument();
 expect(screen.getByRole("button",{name:"Page 2: Check this",pressed:false})).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Page 2: Check this"}));
 expect(screen.getByRole("button",{name:"Page 2: Check this",pressed:true})).toBeInTheDocument();
 expect(screen.getByRole("button",{name:"Page 1: Check this",pressed:false})).toBeInTheDocument();
});
