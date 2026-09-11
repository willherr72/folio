import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {ExistingTextDialog} from "../src/components/ExistingTextDialog";
import {customFontState,ensureCustomFont,fontApi,flushFontReleases,releaseUnownedFont,retainCustomFonts} from "../src/editor/custom-fonts";
const id="e".repeat(64), info={id,name:"DejaVu Serif",weight:400,italic:false,coverage:[[32,126],[233,233]] as [number,number][]};
const run={objectIndex:1,text:"Original",fontName:"ABCDEF+Example",fontSize:14,bounds:{x:30,y:40,width:100,height:15},supported:true,isEmbedded:true,canSubstitute:true};
vi.mock("../src/components/FontPicker",()=>({FontPicker:({onChoose,onClose}:any)=><div role="dialog" aria-label="Substitute"><button onClick={()=>onChoose(info)}>Use previewed substitute</button><button onClick={onClose}>Back to editing</button></div>}));
beforeEach(async()=>{
 vi.stubGlobal("FontFace",class{load(){return Promise.resolve(this);}});
 Object.defineProperty(document,"fonts",{configurable:true,value:{add:vi.fn(),delete:vi.fn()}});
 vi.spyOn(fontApi,"info").mockResolvedValue(info);vi.spyOn(fontApi,"bytes").mockResolvedValue(new Uint8Array([1,2,3]).buffer);vi.spyOn(fontApi,"release").mockResolvedValue();
 await ensureCustomFont(id,info);
});
afterEach(async()=>{cleanup();await act(async()=>{await Promise.resolve();await flushFontReleases();});retainCustomFonts(new Set());await releaseUnownedFont(id);vi.restoreAllMocks();vi.unstubAllGlobals();});
it("retains typed replacement after missing glyph rejection and requires an explicit previewed substitute",async()=>{
 const apply=vi.fn(),cancel=vi.fn();const view=render(<ExistingTextDialog run={run} busy={false} error={null} onApply={apply} onCancel={cancel}/>);
 fireEvent.change(screen.getByRole("textbox",{name:"Replacement text"}),{target:{value:"New café"}});
 view.rerender(<ExistingTextDialog run={run} busy={false} error="The subset lacks é. Choose a substitute font." onApply={apply} onCancel={cancel}/>);
 fireEvent.click(screen.getByRole("button",{name:"Choose substitute font…"}));
 fireEvent.click(screen.getByRole("button",{name:"Use previewed substitute"}));
 expect(screen.getByRole("textbox",{name:"Replacement text"})).toHaveValue("New café");
 expect(screen.getByLabelText("Replacement font preview")).toHaveStyle({fontFamily:"FolioFont_"+id});
 expect(apply).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));expect(apply).toHaveBeenCalledWith("New café",id);
});
it("does not offer substitution for an unsafe layout",()=>{
 render(<ExistingTextDialog run={{...run,supported:false,canSubstitute:false,reason:"Clipping is unsupported"}} busy={false} error={null} onApply={vi.fn()} onCancel={vi.fn()}/>);
 expect(screen.queryByRole("button",{name:"Choose substitute font…"})).not.toBeInTheDocument();
 expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
});
it("keeps a font lease until an in-flight apply finishes even when the dialog unmounts",async()=>{
 let finish!:()=>void;const apply=vi.fn(()=>new Promise<void>(resolve=>finish=resolve));
 const view=render(<ExistingTextDialog run={run} busy={false} error={null} onApply={apply} onCancel={vi.fn()}/>);
 fireEvent.click(screen.getByRole("button",{name:"Choose substitute font…"}));fireEvent.click(screen.getByRole("button",{name:"Use previewed substitute"}));
 fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));view.unmount();
 await act(async()=>{await Promise.resolve();});expect(fontApi.release).not.toHaveBeenCalled();expect(customFontState(id)?.status).toBe("ready");
 await act(async()=>finish());await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(id));
});
