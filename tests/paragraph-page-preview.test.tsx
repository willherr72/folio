import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {ParagraphPagePreview} from "../src/components/ParagraphPagePreview";
import type {FolioAdapter} from "../src/editor/adapter";
import type {EditableTextRun} from "../src/editor/types";

afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
const run:EditableTextRun={objectIndex:7,text:"Visible words",fontName:"Helvetica",fontSize:12,supported:true,bounds:{x:40,y:60,width:100,height:12}};
function options(){return {adapter:{renderPage:vi.fn().mockResolvedValue("data:image/png;base64,AA"),closeDocument:vi.fn()} as unknown as FolioAdapter,
 sourceId:"source",pageIndex:2,pageWidth:600,pageHeight:800,runs:[run],selected:[7],onToggle:vi.fn(),selectable:true};}
it("renders original-page targets in page coordinates with accessible selection",async()=>{
 const props=options();render(<ParagraphPagePreview {...props}/>);
 await screen.findByRole("img",{name:"Original PDF page"});
 expect(props.adapter.renderPage).toHaveBeenCalledWith("source",2,1600);
 const target=screen.getByRole("button",{name:"Select text piece: Visible words"});
 expect(target).toHaveAttribute("aria-pressed","true");expect(target).toHaveAttribute("x","40");
 fireEvent.click(target);expect(props.onToggle).toHaveBeenCalledWith(7);
 fireEvent.keyDown(target,{key:"Enter"});expect(props.onToggle).toHaveBeenCalledTimes(2);
});
it("zooms and compares without mutating or closing the candidate",async()=>{
 const props=options();render(<ParagraphPagePreview {...props} selectable={false} candidateUrl="blob:edited" bounds={{x:40,y:60,width:200,height:80}}/>);
 await screen.findByRole("img",{name:"Paragraph PDF preview"});
 expect(screen.queryByRole("button",{name:"Select text piece: Visible words"})).not.toBeInTheDocument();
 expect(screen.getByLabelText("Paragraph area")).toHaveAttribute("width","200");
 fireEvent.change(screen.getByRole("combobox",{name:"Preview zoom"}),{target:{value:"200"}});
 expect(screen.getByTestId("paragraph-page-surface")).toHaveStyle({width:"200%"});
 fireEvent.click(screen.getByRole("button",{name:"Original"}));await screen.findByRole("img",{name:"Original PDF page"});
 fireEvent.click(screen.getByRole("button",{name:"Edited"}));await screen.findByRole("img",{name:"Paragraph PDF preview"});
 expect(props.adapter.renderPage).toHaveBeenCalledTimes(1);expect(props.adapter.closeDocument).not.toHaveBeenCalled();
});
it("temporarily forces the original without losing the comparison preference",async()=>{
 const props=options();const view=render(<ParagraphPagePreview {...props} selectable={false} candidateUrl="blob:edited"/>);
 await screen.findByRole("img",{name:"Paragraph PDF preview"});
 view.rerender(<ParagraphPagePreview {...props} forceOriginal candidateUrl="blob:edited"/>);
 await screen.findByRole("img",{name:"Original PDF page"});expect(screen.getByRole("button",{name:"Edited"})).toBeDisabled();
 expect(screen.getByRole("button",{name:"Select text piece: Visible words"})).toBeInTheDocument();
 view.rerender(<ParagraphPagePreview {...props} selectable={false} candidateUrl="blob:edited"/>);await screen.findByRole("img",{name:"Paragraph PDF preview"});
 fireEvent.click(screen.getByRole("button",{name:"Original"}));
 view.rerender(<ParagraphPagePreview {...props} forceOriginal candidateUrl="blob:edited"/>);
 view.rerender(<ParagraphPagePreview {...props} selectable={false} candidateUrl="blob:edited"/>);
 expect(screen.getByRole("img",{name:"Original PDF page"})).toBeInTheDocument();expect(props.adapter.closeDocument).not.toHaveBeenCalled();
});
it("releases a late raster URL on unmount without closing the source",async()=>{
 const props=options();let resolve!:(url:string)=>void;
 vi.mocked(props.adapter.renderPage).mockImplementation(()=>new Promise(done=>resolve=done));
 const revoke=vi.fn();vi.stubGlobal("URL",class extends URL {static revokeObjectURL=revoke;});const view=render(<ParagraphPagePreview {...props}/>);view.unmount();
 await act(async()=>resolve("blob:late-original"));
 expect(revoke).toHaveBeenCalledWith("blob:late-original");expect(props.adapter.closeDocument).not.toHaveBeenCalled();
});
it("reports source render failure while leaving candidate comparison usable",async()=>{
 const props=options();vi.mocked(props.adapter.renderPage).mockRejectedValue(new Error("Render unavailable"));
 render(<ParagraphPagePreview {...props} candidateUrl="data:image/png;base64,EDITED"/>);
 await waitFor(()=>expect(screen.getByRole("alert")).toHaveTextContent("Render unavailable"));
 expect(screen.getByRole("img",{name:"Paragraph PDF preview"})).toBeInTheDocument();
 expect(screen.getByRole("button",{name:"Original"})).toBeDisabled();
});
it("disables unselected page targets at the limit while keeping deselection and freed slots usable",async()=>{
 const props=options(),extra={...run,objectIndex:64,text:"One more piece"};
 const selected=Array.from({length:64},(_,index)=>index);
 const view=render(<ParagraphPagePreview {...props} runs={[run,extra]} selected={selected}/>);
 const target=await screen.findByRole("button",{name:"Select text piece: One more piece"});
 expect(target).toHaveAttribute("aria-disabled","true");expect(target).toHaveAttribute("tabindex","-1");
 fireEvent.click(target);fireEvent.keyDown(target,{key:"Enter"});fireEvent.keyDown(target,{key:" "});
 expect(props.onToggle).not.toHaveBeenCalled();
 const removable=screen.getByRole("button",{name:"Select text piece: Visible words"});
 expect(removable).toHaveAttribute("aria-disabled","false");expect(removable).toHaveAttribute("tabindex","0");
 fireEvent.click(removable);expect(props.onToggle).toHaveBeenLastCalledWith(7);
 view.rerender(<ParagraphPagePreview {...props} runs={[run,extra]} selected={selected.filter(index=>index!==7)}/>);
 expect(target).toHaveAttribute("aria-disabled","false");expect(target).toHaveAttribute("tabindex","0");
 fireEvent.keyDown(target,{key:" "});expect(props.onToggle).toHaveBeenLastCalledWith(64);
 expect(props.onToggle).toHaveBeenCalledTimes(2);
});
