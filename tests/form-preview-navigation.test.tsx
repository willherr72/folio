import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {FormFillDialog} from "../src/components/FormFillDialog";
import type {FolioAdapter} from "../src/editor/adapter";
import type {FormField,PagePlan} from "../src/editor/types";
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
const page:PagePlan={id:"p",sourceId:"s",pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
const field:FormField={annotationIndex:0,name:"name",label:"Name",kind:"text",value:{kind:"text",text:"Original"},options:[],selectedIndices:[],required:false,readOnly:false,multiline:false,password:false,multiSelect:false,editableChoice:false,reason:null,visible:true,bounds:{x:40,y:80,width:180,height:30}};
function setup(overrides:Partial<FolioAdapter>={},fields=[field]){
 const adapter:FolioAdapter={kind:"native",openPdf:vi.fn(),exportPdf:vi.fn(),engineStatus:vi.fn(),closeDocument:vi.fn(),renderPage:vi.fn().mockResolvedValue("data:original"),renderPagePlan:vi.fn().mockResolvedValue("data:original"),listFormFields:vi.fn().mockResolvedValue(fields),...overrides};
 const props={page,adapter,pageNumber:1,onApply:vi.fn().mockResolvedValue(undefined),onClose:vi.fn()};return {props,...render(<FormFillDialog {...props}/>)};
}
it("enlarges and sharpens the preview without losing a dirty draft or applying it",async()=>{
 const {props}=setup();const input=await screen.findByRole("textbox",{name:"Name"});await screen.findByAltText("Form page preview");
 fireEvent.change(input,{target:{value:"Unsaved"}});fireEvent.change(screen.getByRole("combobox",{name:"Form preview zoom"}),{target:{value:"3"}});
 await waitFor(()=>expect(props.adapter.renderPagePlan).toHaveBeenLastCalledWith(page,2400));
 expect(screen.getByAltText("Form page preview").parentElement).toHaveStyle({width:"300%"});expect(input).toHaveValue("Unsaved");expect(props.onApply).not.toHaveBeenCalled();
 fireEvent.change(screen.getByRole("combobox",{name:"Form preview zoom"}),{target:{value:"1"}});expect(input).toHaveValue("Unsaved");
});
it("centers the selected target using only the preview scroll position",async()=>{
 setup();await screen.findByAltText("Form page preview");const scroll=screen.getByRole("region",{name:"Scrollable form preview"});
 const target=screen.getByRole("button",{name:"Select form field: Name"});
 vi.spyOn(scroll,"getBoundingClientRect").mockReturnValue({left:10,top:20,width:200,height:100} as DOMRect);
 vi.spyOn(target,"getBoundingClientRect").mockReturnValue({left:310,top:420,width:60,height:20} as DOMRect);
 Object.defineProperties(scroll,{clientWidth:{value:200},clientHeight:{value:100},clientLeft:{value:0},clientTop:{value:0}});
 scroll.scrollLeft=50;scroll.scrollTop=70;const locate=screen.getByRole("button",{name:"Show selected field"});locate.focus();fireEvent.click(locate);
 expect(scroll.scrollLeft).toBe(280);expect(scroll.scrollTop).toBe(430);expect(locate).toHaveFocus();
});
it("disables locate when the list-selected field has no visible target",async()=>{
 setup({},[{...field,visible:false}]);await screen.findByRole("textbox",{name:"Name"});await screen.findByAltText("Form page preview");
 expect(screen.getByRole("button",{name:"Show selected field"})).toBeDisabled();
});
it("keeps the valid raster during sharpening and revokes superseded and late URLs once",async()=>{
 const revoke=vi.fn();vi.stubGlobal("URL",class extends URL {static revokeObjectURL=revoke;});
 let finish!:(url:string)=>void;
 const renderer=vi.fn().mockResolvedValueOnce("blob:initial").mockImplementationOnce(()=>new Promise(resolve=>finish=resolve)).mockResolvedValueOnce("blob:latest");
 const {unmount}=setup({renderPagePlan:renderer});await screen.findByAltText("Form page preview");
 fireEvent.change(screen.getByRole("combobox",{name:"Form preview zoom"}),{target:{value:"2"}});
 expect(screen.getByAltText("Form page preview")).toHaveAttribute("src","blob:initial");expect(revoke).not.toHaveBeenCalled();
 fireEvent.change(screen.getByRole("combobox",{name:"Form preview zoom"}),{target:{value:"3"}});
 await waitFor(()=>expect(screen.getByAltText("Form page preview")).toHaveAttribute("src","blob:latest"));
 await act(async()=>finish("blob:late"));unmount();
 expect(revoke.mock.calls.map(call=>call[0]).sort()).toEqual(["blob:initial","blob:late","blob:latest"]);
});
it("retains the current raster when sharpening fails and hides it immediately on source change",async()=>{
 const renderer=vi.fn().mockResolvedValueOnce("data:initial").mockRejectedValueOnce(new Error("Render failed")).mockImplementation(()=>new Promise(()=>{}));
 const {props,rerender}=setup({renderPagePlan:renderer});await screen.findByAltText("Form page preview");
 fireEvent.change(screen.getByRole("combobox",{name:"Form preview zoom"}),{target:{value:"2"}});
 await screen.findByText("Higher-resolution preview unavailable.");expect(screen.getByAltText("Form page preview")).toHaveAttribute("src","data:initial");
 rerender(<FormFillDialog {...props} page={{...page,sourceId:"new"}}/>);expect(screen.queryByAltText("Form page preview")).not.toBeInTheDocument();expect(screen.queryByRole("button",{name:"Select form field: Name"})).not.toBeInTheDocument();
 expect(screen.getByRole("combobox",{name:"Form preview zoom"})).toHaveValue("2");
});
it("keeps fallback raster and targets in unrotated coordinates at enlarged zoom",async()=>{
 const {props,rerender}=setup({renderPagePlan:undefined});rerender(<FormFillDialog {...props} page={{...page,rotation:90}}/>);
 await screen.findByAltText("Form page preview");fireEvent.change(screen.getByRole("combobox",{name:"Form preview zoom"}),{target:{value:"1.5"}});
 await waitFor(()=>expect(props.adapter.renderPage).toHaveBeenLastCalledWith("s",0,1350));
 expect(screen.getByRole("group",{name:"Interactive form fields"})).toHaveAttribute("viewBox","0 0 300 400");
});
