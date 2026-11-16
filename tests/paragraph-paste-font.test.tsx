import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {ExistingTextDialog} from "../src/components/ExistingTextDialog";
import {drainFontAcquisitions,fontApi,flushFontReleases,releaseUnownedFont,retainCustomFonts} from "../src/editor/custom-fonts";
const id="d".repeat(64),info={id,name:"ASCII font",weight:400,italic:false,coverage:[[32,126]] as [number,number][]};
beforeEach(()=>{
 vi.stubGlobal("FontFace",class{load(){return Promise.resolve(this);}});Object.defineProperty(document,"fonts",{configurable:true,value:{add:vi.fn(),delete:vi.fn()}});
 vi.spyOn(fontApi,"listInstalled").mockResolvedValue([]);vi.spyOn(fontApi,"importFont").mockResolvedValue(info);vi.spyOn(fontApi,"info").mockResolvedValue(info);
 vi.spyOn(fontApi,"bytes").mockResolvedValue(new Uint8Array([1,2]).buffer);vi.spyOn(fontApi,"release").mockResolvedValue();
});
afterEach(async()=>{cleanup();retainCustomFonts(new Set());await act(async()=>{await drainFontAcquisitions();await flushFontReleases();await releaseUnownedFont(id);});vi.restoreAllMocks();vi.unstubAllGlobals();});
function begin(original:string,pasted:string){
 const callback=vi.fn();render(<ExistingTextDialog run={{objectIndex:0,text:original,fontName:"Original",fontSize:12,bounds:{x:0,y:0,width:100,height:20},supported:true,canSubstitute:true}} busy={false} error={null} onApply={vi.fn()} onCancel={vi.fn()} onEditParagraph={callback}/>);
 const input=screen.getByRole("textbox",{name:"Replacement text"}) as HTMLInputElement;input.setSelectionRange(0,original.length);fireEvent.paste(input,{clipboardData:{getData:()=>pasted}});fireEvent.click(screen.getByRole("button",{name:"Choose substitute font…"}));return {input,callback};
}
it("validates and previews staged text after it removes a character absent from the font",async()=>{
 const {callback}=begin("é","first\nsecond");fireEvent.click(screen.getByRole("button",{name:"Import font…"}));
 expect(await screen.findByLabelText("Font preview")).toHaveTextContent("first second");expect(screen.getByLabelText("Font preview").textContent).toBe("first\nsecond");
 fireEvent.click(screen.getByRole("button",{name:"Apply font"}));await screen.findByRole("button",{name:"Paste as paragraph"});expect(screen.getByLabelText("Replacement font preview").textContent).toBe("first\nsecond");expect(screen.queryByRole("alert")).toBeNull();
 fireEvent.click(screen.getByRole("button",{name:"Paste as paragraph"}));expect(callback).toHaveBeenCalledWith(expect.objectContaining({replacement:"first\nsecond",font:expect.objectContaining({id})}));
});
it("rejects a font missing a pasted character without echoing clipboard content, and preserves the stage",async()=>{
 const {callback}=begin("A","é\nnext");fireEvent.click(screen.getByRole("button",{name:"Import font…"}));const error=await screen.findByRole("alert");
 expect(error).toHaveTextContent("pasted text");expect(error.textContent).not.toMatch(/é|U\+00E9/);expect(screen.getByRole("button",{name:"Apply font"})).toBeDisabled();
 await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(id));fireEvent.click(screen.getByRole("button",{name:"Cancel"}));expect(screen.getByRole("textbox",{name:"Replacement text"})).toHaveValue("A");
 fireEvent.click(screen.getByRole("button",{name:"Paste as paragraph"}));expect(callback).toHaveBeenCalledWith(expect.objectContaining({replacement:"é\nnext",font:null}));
});
it("keeps staged text across picker cancel and releases a late font result",async()=>{
 let resolve!:(value:typeof info)=>void;vi.mocked(fontApi.importFont).mockImplementation(()=>new Promise(done=>resolve=done));
 const {callback}=begin("é","first\nsecond");fireEvent.click(screen.getByRole("button",{name:"Import font…"}));await waitFor(()=>expect(fontApi.importFont).toHaveBeenCalled());fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
 await act(async()=>resolve(info));await waitFor(()=>expect(fontApi.release).toHaveBeenCalledWith(id));expect(screen.getByRole("textbox",{name:"Replacement text"})).toHaveValue("é");
 fireEvent.click(screen.getByRole("button",{name:"Paste as paragraph"}));expect(callback).toHaveBeenCalledWith(expect.objectContaining({replacement:"first\nsecond",font:null}));
});
