import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {ExistingTextDialog} from "../src/components/ExistingTextDialog";
import {TextParagraphDialog} from "../src/components/TextParagraphDialog";
import {StyledParagraphDialog} from "../src/components/StyledParagraphDialog";
import type {FolioAdapter} from "../src/editor/adapter";
import type {EditableTextRun} from "../src/editor/types";
import {styledEditorFixture} from "./fixtures/styled-editor-data";
import {fontApi,flushFontReleases,type FontInfo} from "../src/editor/custom-fonts";

vi.mock("../src/components/FontPicker",()=>({FontPicker:({sourceFontName,onClose,onChoose}:{sourceFontName?:string;onClose:()=>void;onChoose:(font:FontInfo)=>void})=><div><p data-testid="source-font">{sourceFontName??"No single source font"}</p><button onClick={()=>onChoose({id:"a".repeat(64),name:"Chosen Substitute",weight:400,italic:false,coverage:[[32,126]]})}>Choose test substitute</button><button onClick={onClose}>Cancel font picker</button></div>}));
beforeEach(()=>{vi.spyOn(fontApi,"release").mockResolvedValue(undefined);});
afterEach(async()=>{cleanup();await flushFontReleases();vi.restoreAllMocks();});
const anchor:EditableTextRun={objectIndex:0,text:"one ",fontName:"ABCDEF+SourceSerif",fontDisplayName:"Source Serif",fontSize:12,bounds:{x:20,y:30,width:30,height:12},supported:true,canSubstitute:true};
const click=()=>fireEvent.click(screen.getByRole("button",{name:"Choose substitute font…"}));
it("passes a single-run PDF display name without losing a typed replacement on cancel",()=>{
 render(<ExistingTextDialog run={anchor} busy={false} error={null} onApply={vi.fn()} onCancel={vi.fn()}/>);
 fireEvent.change(screen.getByRole("textbox",{name:"Replacement text"}),{target:{value:"one this"}});click();expect(screen.getByTestId("source-font")).toHaveTextContent(/^Source Serif$/);
 fireEvent.click(screen.getByRole("button",{name:"Cancel font picker"}));expect(screen.getByRole("textbox",{name:"Replacement text"})).toHaveValue("one this");
});
it("passes the uniform paragraph source name",async()=>{
 const selection={objectIndices:[0],text:"one",fontName:"ABCDEF+SourceSerif",fontDisplayName:"Source Serif",fontSize:12,bounds:{x:20,y:30,width:200,height:100}};
 const adapter={kind:"native",renderPage:vi.fn().mockResolvedValue("data:image/png;base64,AA"),listTextRuns:vi.fn().mockResolvedValue({runs:[anchor]}),suggestTextParagraph:vi.fn().mockResolvedValue(selection),previewTextParagraph:vi.fn(),closeDocument:vi.fn()} as unknown as FolioAdapter;
 render(<TextParagraphDialog adapter={adapter} sourceId="source" pageIndex={0} pageWidth={400} pageHeight={500} overlays={[]} anchor={anchor} onApply={vi.fn()} onCancel={vi.fn()}/>);
 await screen.findByRole("textbox",{name:"Paragraph text"});click();expect(screen.getByTestId("source-font")).toHaveTextContent(/^Source Serif$/);
});
it("retains the PDF source context after explicitly choosing a different substitute",()=>{
 render(<ExistingTextDialog run={{...anchor,fontDisplayName:undefined}} busy={false} error={null} onApply={vi.fn()} onCancel={vi.fn()}/>);
 click();expect(screen.getByTestId("source-font")).toHaveTextContent("ABCDEF+SourceSerif");fireEvent.click(screen.getByRole("button",{name:"Choose test substitute"}));click();expect(screen.getByTestId("source-font")).toHaveTextContent("ABCDEF+SourceSerif");
});
it.each([false,true])("uses the actual selected source style; mixed=%s",async mixed=>{
 const selection=styledEditorFixture();selection.styles[0].fontDisplayName="Source Serif";selection.styles[1].fontDisplayName="Source Sans Bold";
 const adapter={kind:"native",renderPage:vi.fn().mockResolvedValue("data:image/png;base64,AA"),listTextRuns:vi.fn().mockResolvedValue({runs:[anchor]}),suggestStyledParagraph:vi.fn().mockResolvedValue(selection),previewStyledParagraph:vi.fn(),closeDocument:vi.fn()} as unknown as FolioAdapter;
 render(<StyledParagraphDialog adapter={adapter} sourceId="source" pageIndex={0} pageWidth={400} pageHeight={500} overlays={[]} anchor={anchor} onApply={vi.fn()} onCancel={vi.fn()}/>);
 const root=await screen.findByRole("textbox",{name:"Paragraph text"});
 act(()=>{root.focus();const range=document.createRange();range.selectNodeContents(root);if(!mixed){const span=root.querySelector("span")!;range.selectNodeContents(span);}getSelection()!.removeAllRanges();getSelection()!.addRange(range);document.dispatchEvent(new Event("selectionchange"));});
 click();expect(screen.getByTestId("source-font")).toHaveTextContent(mixed?"No single source font":"Source Serif");
});
