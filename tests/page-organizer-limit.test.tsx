import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {createOrganizer} from "../src/editor/page-organizer";
vi.mock("../src/editor/useWorkspaceRecovery",()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:()=>null}));
vi.mock("../src/components/PageOrganizerDialog",()=>({PageOrganizerDialog:({document}:any)=>{createOrganizer(document);return <div role="dialog" aria-label="Organizer limit probe"/>;}}));
afterEach(()=>{cleanup();vi.restoreAllMocks();delete(window as any).__TAURI_INTERNALS__;});
it.each([10000,10001])("handles the organizer entry point with %i pages without losing the document",async count=>{
 localStorage.clear();Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
 vi.spyOn(nativeAdapter,"openPdf").mockResolvedValueOnce({id:"small-source",name:"Small.pdf",pages:[{width:300,height:400}]}).mockResolvedValue({id:"limit-source",name:"Large.pdf",pages:Array.from({length:count},()=>({width:300,height:400}))});
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,AA");vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Small.pdf"});fireEvent.click(screen.getByRole("button",{name:"Bookmarks"}));fireEvent.click(screen.getByRole("button",{name:"Open"}));await screen.findByRole("tab",{name:"Large.pdf"});
 fireEvent.click(screen.getByRole("button",{name:"Organize pages"}));
 if(count===10000)expect(screen.getByRole("dialog",{name:"Organizer limit probe"})).toBeTruthy();
 else{expect(screen.queryByRole("dialog")).toBeNull();expect(screen.getByText("Organizing supports documents with 1 to 10,000 pages.")).toBeVisible();expect(screen.getByRole("tab",{name:"Large.pdf"})).toHaveAttribute("aria-selected","true");expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();expect(screen.queryByLabelText("Unsaved changes")).toBeNull();}
},30000);
