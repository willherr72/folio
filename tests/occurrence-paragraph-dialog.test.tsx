import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {flushSync} from "react-dom";
import {StyledParagraphDialog} from "../src/components/StyledParagraphDialog";
import {TextParagraphDialog} from "../src/components/TextParagraphDialog";
import {createDemoAdapter,type FolioAdapter} from "../src/editor/adapter";
import type {EditorDocument,PageContentReplacement,PagePlan} from "../src/editor/types";
import {styledEditorFixture} from "./fixtures/styled-editor-data";
const size={width:400,height:500},selection=styledEditorFixture();
const anchor={objectIndex:0,text:"one ",fontName:"Courier",fontSize:12,bounds:{x:20,y:30,width:30,height:12},supported:true};
const pages:PagePlan[]=[0,0,1].map((pageIndex,i)=>({id:`p${i}`,sourceId:"source",pageIndex,...size,rotation:0,overlays:[]}));
const document:EditorDocument={name:"Original.pdf",pages,selectedPageId:"p1",selectedOverlayId:null};
const result=():PageContentReplacement=>({document:{id:"preview",name:"Preview.pdf",pages:[size,size,size]},pageIndices:[0,2,1]});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
function setup(styled:boolean,overrides:Partial<FolioAdapter>={},retained=new Set(["source"])){
 const adapter:FolioAdapter={...createDemoAdapter(),renderPage:vi.fn().mockResolvedValue("data:image/png;base64,AA"),renderPagePlan:vi.fn().mockResolvedValue("data:image/png;base64,AA"),closeDocument:vi.fn().mockResolvedValue(undefined),
 listTextRuns:vi.fn().mockRejectedValue(new Error("legacy listing")),listOccurrenceTextRuns:vi.fn().mockResolvedValue({runs:[anchor,{...anchor,objectIndex:1,text:"two"}]}),
 inspectOccurrenceStyledParagraph:vi.fn().mockResolvedValue(selection),suggestOccurrenceStyledParagraph:vi.fn().mockResolvedValue(selection),previewStyledParagraphOccurrence:vi.fn().mockResolvedValue(result()),
 inspectOccurrenceTextParagraph:vi.fn().mockResolvedValue({...selection,fontName:"Courier",fontSize:12}),suggestOccurrenceTextParagraph:vi.fn().mockResolvedValue({...selection,fontName:"Courier",fontSize:12}),previewTextParagraphOccurrence:vi.fn().mockResolvedValue(result()),
 previewStyledParagraph:vi.fn().mockRejectedValue(new Error("legacy preview")),previewTextParagraph:vi.fn().mockRejectedValue(new Error("legacy preview")),...overrides};
 const props={adapter,sourceId:"source",pageIndex:0,pageWidth:400,pageHeight:500,anchor,occurrence:{document,page:pages[1],isSourceRetained:(id:string)=>retained.has(id)},onApply:vi.fn().mockReturnValue(true),onCancel:vi.fn()};
 const Dialog=styled?StyledParagraphDialog:TextParagraphDialog;const view=render(<Dialog {...props}/>);
 return {...view,adapter,props,Dialog,retained};
}
const click=(name:string)=>fireEvent.click(screen.getByRole("button",{name}));
async function ready(){await screen.findByRole("textbox",{name:"Paragraph text"});}
async function preview(){click("Preview paragraph");await screen.findByRole("img",{name:"Paragraph PDF preview"});}
it.each([false,true])("renders and transfers the mapped occurrence (%s styled)",async styled=>{
 const h=setup(styled);await ready();await preview();
 expect(h.adapter.renderPage).toHaveBeenCalledWith("preview",2,1600);
 const method=styled?h.adapter.previewStyledParagraphOccurrence:h.adapter.previewTextParagraphOccurrence;
 expect(vi.mocked(method!).mock.calls[0].slice(0,3)).toEqual(["source",[0,0,1],1]);
 h.props.onApply.mockImplementation(()=>{h.retained.add("preview");flushSync(()=>h.unmount());return true;});click("Apply paragraph");
 expect(h.props.onApply).toHaveBeenCalledWith(result().document,result());expect(h.adapter.closeDocument).not.toHaveBeenCalledWith("preview");
});
it.each([false,true])("rejects borrowed and malformed previews before rendering (%s styled)",async styled=>{
 const h=setup(styled,{},new Set(["source","other-history"]));await ready();
 const method=vi.mocked((styled?h.adapter.previewStyledParagraphOccurrence:h.adapter.previewTextParagraphOccurrence)!);
 method.mockResolvedValueOnce({...result(),document:{...result().document,id:"other-history"}});click("Preview paragraph");await screen.findByRole("alert");
 expect(h.adapter.closeDocument).not.toHaveBeenCalledWith("other-history");expect(h.adapter.renderPage).not.toHaveBeenCalledWith("other-history",expect.anything(),expect.anything());
 method.mockResolvedValueOnce({...result(),pageIndices:[2,0,1]});click("Preview paragraph");await waitFor(()=>expect(h.adapter.closeDocument).toHaveBeenCalledWith("preview"));
 expect(h.adapter.renderPage).not.toHaveBeenCalledWith("preview",expect.anything(),expect.anything());
});
it.each([false,true])("reclaims a late native result after cancellation (%s styled)",async styled=>{
 let finish!:(r:PageContentReplacement)=>void;const h=setup(styled);await ready();
 vi.mocked((styled?h.adapter.previewStyledParagraphOccurrence:h.adapter.previewTextParagraphOccurrence)!).mockImplementation(()=>new Promise(resolve=>finish=resolve));
 click("Preview paragraph");h.unmount();await act(async()=>finish(result()));
 await waitFor(()=>expect(h.adapter.closeDocument).toHaveBeenCalledTimes(1));expect(h.adapter.closeDocument).toHaveBeenCalledWith("preview");
});
it.each([false,true])("reclaims failed raster previews and retains rejected Apply candidates (%s styled)",async styled=>{
 const h=setup(styled);await ready();vi.mocked(h.adapter.renderPage).mockImplementation(async id=>{if(id==="preview")throw new Error("Raster failed");return "data:image/png;base64,AA";});
 click("Preview paragraph");await screen.findByText("Raster failed");expect(h.adapter.closeDocument).toHaveBeenCalledWith("preview");
 vi.mocked(h.adapter.renderPage).mockResolvedValue("data:image/png;base64,AA");await preview();h.props.onApply.mockReturnValue(false);click("Apply paragraph");
 expect(h.adapter.closeDocument).toHaveBeenCalledTimes(1);click("Cancel");await waitFor(()=>expect(h.adapter.closeDocument).toHaveBeenCalledTimes(2));
});
