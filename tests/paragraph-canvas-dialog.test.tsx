import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {StyledParagraphDialog} from "../src/components/StyledParagraphDialog";
import {TextParagraphDialog} from "../src/components/TextParagraphDialog";
import {fontApi,flushFontReleases,type FontInfo} from "../src/editor/custom-fonts";
import {Modal} from "../src/components/Modal";
import {createDemoAdapter} from "../src/editor/adapter";
import type {CanvasParagraphView} from "../src/editor/paragraph-canvas";
import {styledEditorFixture} from "./fixtures/styled-editor-data";
vi.mock("../src/components/FontPicker",()=>({FontPicker:({onClose,onChoose}:{onClose:()=>void;onChoose:(font:FontInfo)=>void})=><Modal title="Test fonts" onClose={onClose}><button onClick={onClose}>Close fonts</button><button onClick={()=>onChoose({id:"b".repeat(64),name:"Canvas font",weight:400,italic:false,coverage:[[32,255]]})}>Choose test font</button></Modal>}));
const selection=styledEditorFixture(),anchor={objectIndex:0,text:"one ",fontName:"Courier",fontSize:12,bounds:{x:20,y:30,width:30,height:12},supported:true};
beforeEach(()=>{vi.spyOn(fontApi,"release").mockResolvedValue(undefined);});
afterEach(async()=>{cleanup();await flushFontReleases();vi.restoreAllMocks();});
it.each(["styled","uniform"])("keeps %s controls in the workspace and updates the complete area",async kind=>{
 let view!:CanvasParagraphView;
 const preview=vi.fn().mockResolvedValue({id:"preview",name:"preview",pages:[{width:400,height:500}]}),adapter={...createDemoAdapter(),listTextRuns:vi.fn().mockResolvedValue({runs:[anchor]}),suggestStyledParagraph:vi.fn().mockResolvedValue(selection),inspectStyledParagraph:vi.fn(),previewStyledParagraph:preview,suggestTextParagraph:vi.fn().mockResolvedValue({...selection,text:"one two",fontName:"Courier",fontSize:12,color:"#000000"}),inspectTextParagraph:vi.fn(),previewTextParagraph:preview,renderPage:vi.fn().mockResolvedValue("data:page"),closeDocument:vi.fn().mockResolvedValue(undefined)};
 const props={adapter,sourceId:"source",pageIndex:0,pageWidth:400,pageHeight:500,anchor,onApply:vi.fn().mockReturnValue(true),onCancel:vi.fn(),canvas:{pageId:"p",sessionKey:"s",zoom:100,onLocate:vi.fn(),onModalChange:vi.fn(),renderCanvas:(v:CanvasParagraphView)=>{view=v;return <div data-testid="main-page"/>;}}};
 const Dialog=kind==="styled"?StyledParagraphDialog:TextParagraphDialog;render(<Dialog {...props}/>);
 await screen.findByRole("textbox",{name:"Paragraph text"});expect(screen.queryByRole("dialog")).toBeNull();await waitFor(()=>expect(view?.enabled).toBe(true));
 act(()=>view.onChange({x:25,y:35,width:180,height:90}));expect(screen.getByRole("spinbutton",{name:"Area width (pt)"})).toHaveValue(180);expect(screen.getByRole("spinbutton",{name:"Area height (pt)"})).toHaveValue(90);
 await waitFor(()=>expect(preview).toHaveBeenCalledOnce());await waitFor(()=>expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeEnabled());
 fireEvent.change(screen.getByRole("spinbutton",{name:"Left (pt)"}),{target:{value:""}});expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
});

it.each(["styled","uniform"])("serializes %s native work and keeps the canvas mounted under the font modal",async kind=>{
 let view!:CanvasParagraphView,finish!:(value:any)=>void;
 const preview=vi.fn().mockImplementation(()=>new Promise(resolve=>finish=resolve)),closed=vi.fn().mockResolvedValue(undefined);
 const adapter={...createDemoAdapter(),listTextRuns:vi.fn().mockResolvedValue({runs:[anchor]}),suggestStyledParagraph:vi.fn().mockResolvedValue(selection),inspectStyledParagraph:vi.fn(),previewStyledParagraph:preview,suggestTextParagraph:vi.fn().mockResolvedValue({...selection,text:"one two",fontName:"Courier",fontSize:12,color:"#000000"}),inspectTextParagraph:vi.fn(),previewTextParagraph:preview,renderPage:vi.fn().mockResolvedValue("data:page"),closeDocument:closed};
 const modal=vi.fn(),props={adapter,sourceId:"source",pageIndex:0,pageWidth:400,pageHeight:500,anchor,onApply:vi.fn().mockReturnValue(true),onCancel:vi.fn(),canvas:{pageId:"p",sessionKey:"s",zoom:100,onLocate:vi.fn(),onModalChange:modal,renderCanvas:(v:CanvasParagraphView)=>{view=v;return <div data-testid="main-page"/>;}}};
 const Dialog=kind==="styled"?StyledParagraphDialog:TextParagraphDialog;render(<Dialog {...props}/>);await screen.findByRole("textbox",{name:"Paragraph text"});await waitFor(()=>expect(view.enabled).toBe(true));
 act(()=>view.onChange({...selection.bounds,width:240}));
 const node=screen.getByTestId("main-page");fireEvent.click(screen.getByRole("button",{name:"Choose substitute font…"}));expect(screen.getByRole("dialog",{name:"Test fonts"})).toBeInTheDocument();expect(screen.getByTestId("main-page")).toBe(node);expect(modal).toHaveBeenLastCalledWith(true);
 await act(async()=>{await new Promise(resolve=>setTimeout(resolve,400));});expect(preview).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"Choose test font"}));expect(screen.queryByRole("dialog")).toBeNull();expect(props.onCancel).not.toHaveBeenCalled();expect(modal).toHaveBeenLastCalledWith(false);
 fireEvent.click(screen.getByRole("button",{name:"Preview paragraph"}));fireEvent.change(screen.getByRole("spinbutton",{name:"Left (pt)"}),{target:{value:"21"}});
 expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeDisabled();expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();expect(preview).toHaveBeenCalledOnce();
 await act(async()=>finish({id:"stale",name:"preview",pages:[{width:400,height:500}]}));await waitFor(()=>expect(closed).toHaveBeenCalledWith("stale"));expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeEnabled();expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
 await act(async()=>{await new Promise(resolve=>setTimeout(resolve,450));});expect(preview).toHaveBeenCalledOnce();
});
