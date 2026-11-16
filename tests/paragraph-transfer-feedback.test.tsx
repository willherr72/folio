import {act,cleanup,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {StyledParagraphDialog} from "../src/components/StyledParagraphDialog";
import {createDemoAdapter} from "../src/editor/adapter";
import {createStyledEditor} from "../src/editor/styled-paragraph-editor";
import {styledEditorFixture} from "./fixtures/styled-editor-data";
import type {ParagraphDraftFitFeedback} from "../src/editor/paragraph-draft-fit";
afterEach(cleanup);
function setup(feedback:ParagraphDraftFitFeedback,listFailure=false){
 const selected=styledEditorFixture();const anchor={objectIndex:0,text:"one ",fontName:"Courier",fontSize:12,bounds:selected.bounds,supported:true};
 const adapter={...createDemoAdapter(),listTextRuns:listFailure?vi.fn().mockRejectedValue(new Error("Current selection could not be read.")):vi.fn().mockResolvedValue({runs:[anchor]}),suggestStyledParagraph:vi.fn().mockResolvedValue(selected),inspectStyledParagraph:vi.fn().mockResolvedValue(selected),fitStyledParagraph:vi.fn().mockResolvedValue(selected.bounds),previewStyledParagraph:vi.fn().mockResolvedValue({id:"preview",name:"Preview",pages:[{width:400,height:500}]}),closeDocument:vi.fn().mockResolvedValue(undefined)};
 render(<StyledParagraphDialog adapter={adapter} sourceId="source" pageIndex={0} pageWidth={400} pageHeight={500} anchor={anchor} initialDraft={{kind:"styled",editor:createStyledEditor(selected),font:null,fitFeedback:feedback}} onApply={vi.fn()} onCancel={vi.fn()}/>);
 return adapter;
}
it("shows successful transfer fitting as a proposal that still needs Preview",async()=>{
 setup({kind:"fitted",message:"Area fitted to the transferred text. Preview before applying."});expect(await screen.findByRole("textbox",{name:"Paragraph text"})).toHaveTextContent("one two");expect(screen.getByRole("status")).toHaveTextContent("Area fitted");expect(screen.getByRole("button",{name:"Apply paragraph"})).toBeDisabled();
});
it("keeps a nonfatal automatic-fit notice separate from an active selection failure",async()=>{
 setup({kind:"unavailable",message:"Automatic fitting could not find a clear area."},true);await screen.findByRole("textbox",{name:"Paragraph text"});
 expect(screen.getByRole("alert")).toHaveTextContent("Current selection could not be read");expect(screen.getByRole("status")).toHaveTextContent("Automatic fitting could not find");
});
it("preserves manual correction controls when automatic fitting was unavailable",async()=>{
 setup({kind:"unavailable",message:"Your transferred text is preserved; adjust the area."});await screen.findByRole("textbox",{name:"Paragraph text"});expect(screen.queryByRole("alert")).toBeNull();expect(screen.getByRole("status")).toHaveTextContent("transferred text is preserved");expect(screen.getByRole("button",{name:"Fit area to text"})).toBeEnabled();expect(screen.getByRole("button",{name:"Preview paragraph"})).toBeEnabled();
});
