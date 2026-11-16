import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {recoveryApi} from "../src/editor/recovery";
import {fontApi} from "../src/editor/custom-fonts";
import {clearSearchTextCache} from "../src/editor/search";

const nativeWindow=vi.hoisted(()=>({listen:vi.fn().mockResolvedValue(()=>{})}));
vi.mock("@tauri-apps/api/window",()=>({getCurrentWindow:()=>({onCloseRequested:nativeWindow.listen})}));
beforeEach(()=>{
 localStorage.clear();Object.defineProperty(window,"__TAURI_INTERNALS__",{configurable:true,value:{}});
 vi.spyOn(fontApi,"release").mockResolvedValue();vi.spyOn(recoveryApi,"load").mockResolvedValue(null);vi.spyOn(recoveryApi,"save").mockResolvedValue();vi.spyOn(recoveryApi,"clear").mockResolvedValue();
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValueOnce({id:"find-options-one",name:"Find One.pdf",pages:[{width:300,height:400}]}).mockResolvedValueOnce({id:"find-options-two",name:"Find Two.pdf",pages:[{width:300,height:400}]});
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"renderPagePlan").mockResolvedValue("data:image/png;base64,");
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"getPageText").mockResolvedValue({characters:Array.from("foo Foo foobar foo",(text,index)=>({text,x:10+index*6,y:30,width:6,height:12}))});
 vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
});
afterEach(()=>{cleanup();clearSearchTextCache(["find-options-one","find-options-two"]);delete (window as unknown as Record<string,unknown>).__TAURI_INTERNALS__;vi.restoreAllMocks();});

it("keeps flags and selected result per tab, resets after option edits and supports document F3 wrapping without dirtying PDFs",async()=>{
 render(<App initialDemo={false}/>);await waitFor(()=>expect(screen.getByRole("button",{name:"Open a PDF"})).toBeEnabled());
 fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Find One.pdf"});
 fireEvent.keyDown(window,{key:"f",ctrlKey:true});fireEvent.change(screen.getByRole("searchbox"),{target:{value:"foo"}});await screen.findByText("1 of 4");
 fireEvent.keyDown(window,{key:"F3"});expect(screen.getByText("2 of 4")).toBeInTheDocument();
 fireEvent.click(screen.getByRole("checkbox",{name:"Match case"}));await screen.findByText("1 of 3");
 fireEvent.click(screen.getByRole("checkbox",{name:"Whole words"}));await screen.findByText("1 of 2");
 const input=screen.getByRole("searchbox")as HTMLInputElement;
 screen.getByRole("button",{name:"Find"}).focus();
 fireEvent.keyDown(window,{key:"f",ctrlKey:true});expect(input).toHaveFocus();expect([input.selectionStart,input.selectionEnd]).toEqual([0,3]);
 screen.getByRole("checkbox",{name:"Whole words"}).focus();
 fireEvent.keyDown(screen.getByRole("checkbox",{name:"Whole words"}),{key:"f",ctrlKey:true});expect(input).toHaveFocus();
 fireEvent.keyDown(screen.getByRole("searchbox"),{key:"F3",shiftKey:true});expect(screen.getByText("2 of 2")).toBeInTheDocument();
 fireEvent.keyDown(window,{key:"F3",isComposing:true});expect(screen.getByText("2 of 2")).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Open"}));await screen.findByRole("tab",{name:"Find Two.pdf"});
 fireEvent.click(screen.getByRole("button",{name:"Find"}));expect(screen.getByRole("checkbox",{name:"Match case"})).not.toBeChecked();expect(screen.getByRole("checkbox",{name:"Whole words"})).not.toBeChecked();
 fireEvent.change(screen.getByRole("searchbox"),{target:{value:"Foo"}});await screen.findByText("1 of 4");
 fireEvent.keyDown(screen.getByRole("searchbox"),{key:"Tab",ctrlKey:true});expect(screen.getByRole("searchbox")).toHaveValue("foo");expect(screen.getByRole("checkbox",{name:"Match case"})).toBeChecked();expect(screen.getByRole("checkbox",{name:"Whole words"})).toBeChecked();await screen.findByText("2 of 2");
 fireEvent.keyDown(screen.getByRole("checkbox",{name:"Whole words"}),{key:"Escape"});expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Find"}));await screen.findByText("2 of 2");expect(screen.getByRole("searchbox")).toHaveFocus();
 expect(screen.queryByLabelText("Unsaved changes")).not.toBeInTheDocument();
});
