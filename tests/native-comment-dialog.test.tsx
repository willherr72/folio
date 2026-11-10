import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {NativeCommentDialog} from "../src/components/NativeCommentDialog";
import type {NativeComment,PagePlan} from "../src/editor/types";
const comment:NativeComment={pageIndex:0,annotationIndex:0,text:"Original",bounds:{x:30,y:40,width:18,height:18},color:"#ffcc00",canChangeColor:true};
const page:PagePlan={id:"page",sourceId:"source",pageIndex:0,width:300,height:400,rotation:90,overlays:[]};
afterEach(cleanup);
it("edits position in displayed rotated-page coordinates and sends one combined change",async()=>{
 const apply=vi.fn().mockResolvedValue(undefined);
 render(<NativeCommentDialog comment={comment} page={page} pageNumber={1} onApply={apply} onDelete={vi.fn()} onClose={vi.fn()}/>);
 expect(screen.getByRole("spinbutton",{name:"Left (pt)"})).toHaveValue(342);expect(screen.getByRole("spinbutton",{name:"Top (pt)"})).toHaveValue(30);
 fireEvent.change(screen.getByRole("spinbutton",{name:"Left (pt)"}),{target:{value:"320"}});fireEvent.change(screen.getByRole("spinbutton",{name:"Top (pt)"}),{target:{value:"50"}});
 fireEvent.change(screen.getByLabelText("Comment color"),{target:{value:"#123456"}});fireEvent.change(screen.getByRole("textbox",{name:"Comment text"}),{target:{value:"Moved"}});
 fireEvent.click(screen.getByRole("button",{name:"Apply comment"}));await waitFor(()=>expect(apply).toHaveBeenCalledWith({text:"Moved",position:{x:50,y:62},color:"#123456"}));
});
it("keeps custom appearance colors and comment threads protected while allowing text edits",()=>{
 render(<NativeCommentDialog comment={{...comment,canChangeColor:false,deleteReason:"This comment has replies."}} page={page} pageNumber={1} onApply={vi.fn()} onDelete={vi.fn()} onClose={vi.fn()}/>);
 expect(screen.getByLabelText("Comment color")).toBeDisabled();expect(screen.getByRole("button",{name:"Delete comment"})).toBeDisabled();expect(screen.getByText(/custom appearance/i)).toBeInTheDocument();expect(screen.getByText("This comment has replies.")).toBeInTheDocument();
 fireEvent.change(screen.getByRole("textbox",{name:"Comment text"}),{target:{value:"Changed"}});expect(screen.getByRole("button",{name:"Apply comment"})).toBeEnabled();
});
it("rejects blank or outside-page positions and retains drafts when deletion fails",async()=>{
 const remove=vi.fn().mockRejectedValue(new Error("Cannot delete this comment"));
 render(<NativeCommentDialog comment={comment} page={page} pageNumber={1} onApply={vi.fn()} onDelete={remove} onClose={vi.fn()}/>);
 fireEvent.change(screen.getByRole("spinbutton",{name:"Left (pt)"}),{target:{value:""}});expect(screen.getByRole("button",{name:"Apply comment"})).toBeDisabled();
 fireEvent.change(screen.getByRole("spinbutton",{name:"Left (pt)"}),{target:{value:"400"}});expect(screen.getByRole("button",{name:"Apply comment"})).toBeDisabled();
 fireEvent.click(screen.getByRole("button",{name:"Delete comment"}));expect(await screen.findByRole("alert")).toHaveTextContent("Cannot delete this comment");expect(screen.getByRole("textbox",{name:"Comment text"})).toHaveValue("Original");
});
it("preserves a transparent or unspecified native color when only text changes",async()=>{
 const apply=vi.fn().mockResolvedValue(undefined);
 render(<NativeCommentDialog comment={{...comment,color:null}} page={page} pageNumber={1} onApply={apply} onClose={vi.fn()}/>);
 expect(screen.getByRole("button",{name:"Apply comment"})).toBeDisabled();
 fireEvent.change(screen.getByRole("textbox",{name:"Comment text"}),{target:{value:"Text only"}});fireEvent.click(screen.getByRole("button",{name:"Apply comment"}));
 await waitFor(()=>expect(apply).toHaveBeenCalledWith({text:"Text only"}));
});
