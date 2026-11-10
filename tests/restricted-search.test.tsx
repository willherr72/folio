import {act,cleanup,render,renderHook,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {createDemoAdapter} from "../src/editor/adapter";
import type {PagePlan} from "../src/editor/types";
import {useDocumentSearch} from "../src/editor/search";
import {PdfTextLayer} from "../src/components/PdfTextLayer";
const page:PagePlan={id:"page",sourceId:"protected",pageIndex:0,width:300,height:400,rotation:0,overlays:[],protection:{documentId:"origin",revision:4,owner:false,permissions:{copyContent:false,editContent:false,annotate:true,fillForms:true,assemble:false,print:true,printHighQuality:false}}};
afterEach(cleanup);
it("finds restricted source and overlay text without requesting or rendering the source characters",async()=>{
 const api={...createDemoAdapter(),getPageText:vi.fn().mockResolvedValue({characters:[{text:"Private source text",x:10,y:10,width:100,height:10}]}),searchPage:vi.fn().mockResolvedValue({matches:[[{x:20,y:30,width:40,height:12}]],hasText:true,truncated:false})};
 const annotated:PagePlan={...page,overlays:[{type:"text",id:"note",text:"Needle",x:10,y:100,fontSize:12,color:"#000000"}]};
 const pages=[annotated,{...annotated,id:"duplicate"}];
 const hook=renderHook(()=>useDocumentSearch(api,pages,"Needle",true));
 render(<svg><PdfTextLayer adapter={api} page={page} pageNumber={1} selectable={true}/></svg>);
 await waitFor(()=>expect(hook.result.current.searching).toBe(false));
 expect(hook.result.current.matches).toHaveLength(4);expect(hook.result.current.hasText).toBe(true);
 expect(api.searchPage).toHaveBeenCalledWith("protected",0,"needle");expect(api.searchPage).toHaveBeenCalledTimes(1);
 expect(api.getPageText).not.toHaveBeenCalled();expect(screen.queryByText("Private source text")).not.toBeInTheDocument();
});
it("reports result limits and ignores stale restricted results after a query change",async()=>{
 let old!:(value:any)=>void;
 const api={...createDemoAdapter(),getPageText:vi.fn(),searchPage:vi.fn().mockImplementationOnce(()=>new Promise(resolve=>{old=resolve;})).mockResolvedValue({matches:[[{x:10,y:20,width:30,height:10}]],hasText:true,truncated:true})};
 const pages=[page];const hook=renderHook(({query})=>useDocumentSearch(api,pages,query,true),{initialProps:{query:"old"}});
 await waitFor(()=>expect(old).toBeTypeOf("function"));hook.rerender({query:"new"});
 await waitFor(()=>expect(hook.result.current.searching).toBe(false));expect(hook.result.current.error).toMatch(/limit|refine/i);
 await act(async()=>old({matches:[],hasText:false,truncated:false}));expect(hook.result.current.matches).toHaveLength(1);
 expect(api.getPageText).not.toHaveBeenCalled();
});
