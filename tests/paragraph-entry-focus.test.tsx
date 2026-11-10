import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {StyledParagraphDialog} from "../src/components/StyledParagraphDialog";
import {TextParagraphDialog} from "../src/components/TextParagraphDialog";
import {createDemoAdapter} from "../src/editor/adapter";
import {clearRenderCache} from "../src/components/PageView";
import {styledEditorFixture} from "./fixtures/styled-editor-data";

afterEach(()=>{cleanup();clearRenderCache(["entry-source"]);vi.restoreAllMocks();});
const selection=styledEditorFixture();
const anchor={objectIndex:0,text:"one ",fontName:"Courier",fontSize:12,bounds:{x:20,y:30,width:30,height:12},supported:true};
function setup(kind:string){
 let resolve!:(value:any)=>void,reject!:(reason:Error)=>void;
 const pending=new Promise<any>((yes,no)=>{resolve=yes;reject=no;});
 const adapter={...createDemoAdapter(),listTextRuns:vi.fn().mockResolvedValue({runs:[anchor]}),suggestStyledParagraph:()=>pending,suggestTextParagraph:()=>pending,renderPage:vi.fn().mockResolvedValue("data:page")};
 const props={adapter,sourceId:"entry-source",pageIndex:0,pageWidth:400,pageHeight:500,anchor,onApply:vi.fn().mockReturnValue(true),onCancel:vi.fn(),canvas:{pageId:"p",sessionKey:"s",zoom:100,onLocate:vi.fn(),onModalChange:vi.fn(),renderCanvas:()=> <div/>}};
 const Dialog=kind==="styled"?StyledParagraphDialog:TextParagraphDialog;
 const result=render(<><button>Outside control</button><Dialog {...props}/></>);
 return {...result,finish:()=>resolve(kind==="styled"?selection:{...selection,text:"one two",fontName:"Courier",fontSize:12,color:"#000000"}),reject};
}
it.each(["styled","uniform"])("focuses the %s workspace while loading, then its paragraph text",async kind=>{
 const {finish}=setup(kind);
 expect(screen.getByRole("region",{name:"Edit paragraph"})).toHaveFocus();
 await act(async()=>finish());
 expect(await screen.findByRole("textbox",{name:"Paragraph text"})).toHaveFocus();
 const width=screen.getByRole("spinbutton",{name:"Area width (pt)"});width.focus();
 fireEvent.change(width,{target:{value:"180"}});
 await waitFor(()=>expect(width).toHaveFocus());
});
it.each(["styled","uniform"])("focuses %s selection controls when initial inspection is refused",async kind=>{
 const {reject}=setup(kind);
 await act(async()=>reject(new Error("Select the paragraph pieces.")));
 expect(await screen.findByRole("group",{name:/Paragraph pieces/})).toHaveFocus();
 expect(screen.getByRole("alert")).toHaveTextContent("Select the paragraph pieces.");
});
it.each(["styled","uniform"])("does not steal focus moved outside the %s panel during loading",async kind=>{
 const {finish}=setup(kind);const outside=screen.getByRole("button",{name:"Outside control"});outside.focus();
 await act(async()=>finish());await screen.findByRole("textbox",{name:"Paragraph text"});
 expect(outside).toHaveFocus();
});
it.each(["styled","uniform"])("does not steal focus moved to a %s panel control during loading",async kind=>{
 const {finish}=setup(kind);const locate=screen.getByRole("button",{name:"Locate paragraph"});locate.focus();
 await act(async()=>finish());await screen.findByRole("textbox",{name:"Paragraph text"});
 expect(locate).toHaveFocus();
});
it.each(["styled","uniform"])("does not reclaim focus after the %s workspace unmounts",async kind=>{
 const {finish,unmount}=setup(kind);unmount();
 const outside=document.createElement("button");document.body.append(outside);outside.focus();
 try{await act(async()=>finish());expect(outside).toHaveFocus();}finally{outside.remove();}
});
