import {cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter,documentToPages} from "../src/editor/adapter";
import {PrintDialog} from "../src/components/PrintDialog";
import {fontApi} from "../src/editor/custom-fonts";
import type {DocumentInfo} from "../src/editor/types";
const source:DocumentInfo={id:"restricted",name:"Protected.pdf",pages:[{width:300,height:400,overlays:[{type:"comment",id:"note",x:20,y:20,text:"Review me",color:"#ffff00"},{type:"text",id:"caption",x:20,y:100,text:"Keep caption",fontSize:12,color:"#000000"}]},{width:300,height:400}],bookmarks:[{id:"bookmark",title:"First page",level:0,destination:{kind:"array",value:[{kind:"page",value:"0"},{kind:"name",value:[70,105,116]}]}}],protection:{documentId:"origin",revision:4,owner:false,permissions:{copyContent:false,editContent:false,annotate:false,fillForms:false,assemble:false,print:false,printHighQuality:false}}};
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:(props:any)=><div><output data-testid="pages">{JSON.stringify(props.pages)}</output><button onClick={()=>props.onSelectOverlay(props.pages[0].id,"note")}>Review note</button><button onClick={()=>props.onAddText(props.pages[0].id,{x:30,y:30})}>Stale text callback</button></div>}));
beforeEach(()=>{localStorage.clear();vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue(source);vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");vi.spyOn(nativeAdapter,"exportPdf").mockResolvedValue("Saved.pdf");vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Protected.pdf"});}
it("explains restrictions, disables disallowed actions, and keeps viewing, review and saving available",async()=>{
 await open();
 for(const name of ["Edit text","Fill forms","Text","Text area","Draw","Signature","Highlight","Comment","Add PDF","Rotate","Duplicate","Move down","Delete page","Print","Insert blank page…","Insert image pages…"])expect(screen.getByRole("button",{name})).toBeDisabled();
 expect(screen.getByLabelText("PDF permissions")).toHaveTextContent(/copying text/i);
 expect(screen.getByRole("button",{name:"Find"})).toBeEnabled();expect(screen.getByRole("button",{name:"Save a copy"})).toBeEnabled();
 fireEvent.click(screen.getByRole("button",{name:"Next page"}));expect(screen.getByRole("button",{name:"Go to page"})).toHaveTextContent("Page 2 of 2");
 fireEvent.click(screen.getByRole("button",{name:"Previous page"}));expect(screen.getByRole("button",{name:"Go to page"})).toHaveTextContent("Page 1 of 2");
 expect(screen.getByRole("button",{name:"Page 1 of 2"})).toHaveAttribute("draggable","false");
 fireEvent.click(screen.getByRole("button",{name:"Save options"}));expect(screen.getByRole("menuitem",{name:"Flatten text, images, and ink"})).toBeDisabled();expect(screen.getByRole("menuitem",{name:"Extract pages…"})).toBeDisabled();
 fireEvent.keyDown(screen.getByRole("menu",{name:"Save options"}),{key:"Escape"});
 const before=screen.getByTestId("pages").textContent;fireEvent.click(screen.getByText("Review note"));
 expect(screen.getByDisplayValue("Review me")).toBeDisabled();fireEvent.keyDown(window,{key:"Delete"});fireEvent.click(screen.getByText("Stale text callback"));expect(screen.getByTestId("pages").textContent).toBe(before);
 fireEvent.click(screen.getByRole("button",{name:"Bookmarks"}));expect(screen.getByRole("button",{name:"Add bookmark…"})).toBeDisabled();
 fireEvent.click(screen.getByRole("treeitem",{name:/First page/}));expect(screen.getByRole("treeitem",{name:/First page/})).toHaveAttribute("aria-selected","true");
 expect(screen.getByRole("button",{name:"Edit bookmark"})).toBeDisabled();
 fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalled());
});
it("annotation permission does not enable content editing, page assembly or flattening",async()=>{
 vi.mocked(nativeAdapter.openPdf).mockResolvedValue({...source,protection:{...source.protection!,permissions:{...source.protection!.permissions,annotate:true,fillForms:true,print:true}}});await open();
 for(const name of ["Text","Draw","Comment","Fill forms","Print"])expect(screen.getByRole("button",{name})).toBeEnabled();
 for(const name of ["Edit text","Rotate","Duplicate","Highlight"])expect(screen.getByRole("button",{name})).toBeDisabled();
 fireEvent.click(screen.getByRole("button",{name:"Save options"}));expect(within(screen.getByRole("menu")).getByRole("menuitem",{name:"Flatten text, images, and ink"})).toBeDisabled();
});

it("refuses incompatible protected additions before committing and releases the new source",async()=>{
 const unlocked={...source,protection:{...source.protection!,owner:true,permissions:{copyContent:true,editContent:true,annotate:true,fillForms:true,assemble:true,print:true,printHighQuality:true}}};
 const fontId="b".repeat(64);const release=vi.spyOn(fontApi,"release").mockResolvedValue();
 vi.mocked(nativeAdapter.openPdf).mockResolvedValueOnce(unlocked).mockResolvedValueOnce({...unlocked,id:"different",name:"Other.pdf",pages:[{width:300,height:400,overlays:[{type:"text",id:"t",fontId,text:"note",x:10,y:20,fontSize:12,color:"#000000"}]}]});
 const check=vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"checkSourceCombination").mockRejectedValue(new Error("These PDFs use different protection. Save separate protected copies."));
 await open();const before=screen.getByTestId("pages").textContent;fireEvent.click(screen.getByRole("button",{name:"Add PDF"}));
 await waitFor(()=>expect(check).toHaveBeenCalledWith(["restricted","different"]));
 await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("different"));expect(screen.getByTestId("pages").textContent).toBe(before);
 expect(screen.getByRole("alert")).toHaveTextContent("different protection");
 await waitFor(()=>expect(release).toHaveBeenCalledWith(fontId));
});

it("allows printing a permitted subset and explains its quality limit",()=>{
 const pages=documentToPages(source);
 pages[1]={...pages[1],protection:{...source.protection!,permissions:{...source.protection!.permissions,print:true}}};
 const print=vi.fn();render(<PrintDialog pages={pages} currentPageId={pages[1].id} onClose={()=>{}} onPrint={print}/>);
 expect(screen.getByRole("button",{name:"Choose printer…"})).toBeDisabled();expect(screen.getByRole("alert")).toHaveTextContent("do not allow printing");
 fireEvent.change(screen.getByLabelText("Pages"),{target:{value:"current"}});
 expect(screen.getByRole("button",{name:"Choose printer…"})).toBeEnabled();expect(screen.getByText(/limits printing to 150 dpi/)).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Choose printer…"}));expect(print).toHaveBeenCalledWith([pages[1]],{scale:"fit",orientation:"portrait"});
 fireEvent.change(screen.getByLabelText("Pages"),{target:{value:"range"}});fireEvent.change(screen.getByLabelText("Page range"),{target:{value:"1-2"}});
 expect(screen.getByRole("button",{name:"Choose printer…"})).toBeDisabled();
});

it("releases an incoming source when the app closes during combination validation",async()=>{
 const owner={...source,protection:{...source.protection!,owner:true,permissions:{copyContent:true,editContent:true,annotate:true,fillForms:true,assemble:true,print:true,printHighQuality:true}}};
 vi.mocked(nativeAdapter.openPdf).mockResolvedValueOnce(owner).mockResolvedValueOnce({...owner,id:"late"});
 let finish!:()=>void;const check=vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"checkSourceCombination").mockImplementation(()=>new Promise<void>(resolve=>{finish=resolve;}));
 const view=render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Protected.pdf"});
 fireEvent.click(screen.getByRole("button",{name:"Add PDF"}));await waitFor(()=>expect(check).toHaveBeenCalled());view.unmount();finish();
 await waitFor(()=>expect(nativeAdapter.closeDocument).toHaveBeenCalledWith("late"));
});
