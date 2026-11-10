import {act,cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {App} from "../src/App";
import {nativeAdapter} from "../src/editor/adapter";
import {bookmarkPage} from "../src/editor/bookmarks";
import {clearSearchTextCache} from "../src/editor/search";
import type {DocumentBookmark,PdfValue} from "../src/editor/types";
vi.mock("../src/components/DocumentViewport",()=>({DocumentViewport:()=>null}));
const fit=(page:string):PdfValue=>({kind:"array",value:[{kind:"page",value:page},{kind:"name",value:[70,105,116]}]});
const bookmarks:DocumentBookmark[]=[{id:"a",title:"Chapter",level:0,destination:fit("1")},{id:"b",title:"Section",level:1,destination:fit("2")},{id:"c",title:"Opening",level:0,destination:fit("0")}];
beforeEach(()=>{
 localStorage.clear();vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"source",name:"Book.pdf",pages:Array.from({length:3},()=>({width:300,height:400})),bookmarks});
 vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");vi.spyOn(nativeAdapter,"exportPdf").mockResolvedValue("Saved.pdf");vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
});
afterEach(()=>{cleanup();clearSearchTextCache(["source","other"]);vi.restoreAllMocks();});
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));await screen.findByRole("tab",{name:"Book.pdf"});fireEvent.click(screen.getByRole("button",{name:"Bookmarks"}));}
it("navigates imported nested bookmarks without changing dirty status and exports mapped targets after page moves",async()=>{
 await open();fireEvent.click(screen.getByRole("treeitem",{name:/^Chapter/}));expect(screen.getByText("Page 2 of 3")).toBeInTheDocument();expect(screen.queryByLabelText("Unsaved changes")).not.toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Move down"}));fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalled());
 const [pages,options]=vi.mocked(nativeAdapter.exportPdf).mock.calls[0];expect(pages.map(p=>p.pageIndex)).toEqual([0,2,1]);expect(bookmarkPage(options!.bookmarks![0])).toBe(pages[2].id);
});
it("creates a child, renames it without changing its destination, and undoes bookmark edits",async()=>{
 await open();fireEvent.click(screen.getByRole("treeitem",{name:/^Chapter/}));fireEvent.click(screen.getByRole("button",{name:"Add child bookmark"}));
 fireEvent.change(screen.getByRole("textbox",{name:"Title"}),{target:{value:"My section"}});fireEvent.change(screen.getByRole("combobox",{name:"Destination"}),{target:{value:"2"}});fireEvent.click(screen.getByRole("button",{name:"Add bookmark"}));
 await screen.findByRole("treeitem",{name:/^My section/});expect(screen.getByLabelText("Unsaved changes")).toBeInTheDocument();
 fireEvent.click(screen.getByRole("treeitem",{name:/^My section/}));fireEvent.click(screen.getByRole("button",{name:"Edit bookmark"}));fireEvent.change(screen.getByRole("textbox",{name:"Title"}),{target:{value:"Renamed"}});fireEvent.click(screen.getByRole("button",{name:"Save bookmark"}));
 await screen.findByRole("treeitem",{name:/^Renamed/});fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByRole("treeitem",{name:/^My section/})).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.queryByRole("treeitem",{name:/^My section/})).not.toBeInTheDocument();expect(screen.queryByLabelText("Unsaved changes")).not.toBeInTheDocument();
});
it("extracts only valid destinations, keeps parent headings, and saves an explicit empty tree after deletion",async()=>{
 await open();fireEvent.click(screen.getByRole("treeitem",{name:/^Section/}));fireEvent.click(screen.getByRole("button",{name:"Extract pages…"}));fireEvent.click(screen.getByRole("button",{name:"Save extracted PDF…"}));await waitFor(()=>expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
 const [pages,options]=vi.mocked(nativeAdapter.exportPdf).mock.calls[0];expect(options!.bookmarks!.map(b=>b.title)).toEqual(["Chapter","Section"]);expect(bookmarkPage(options!.bookmarks![0])).toBeNull();expect(bookmarkPage(options!.bookmarks![1])).toBe(pages[0].id);
 fireEvent.click(screen.getByRole("treeitem",{name:/^Chapter/}));fireEvent.click(screen.getByRole("button",{name:"Delete bookmark"}));expect(screen.queryByRole("treeitem",{name:/^Section/})).not.toBeInTheDocument();
 fireEvent.click(screen.getByRole("treeitem",{name:/^Opening/}));fireEvent.click(screen.getByRole("button",{name:"Delete bookmark"}));fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalledTimes(2));expect(vi.mocked(nativeAdapter.exportPdf).mock.calls[1][1]).toEqual({bookmarks:[]});
});
it("appends imported trees with destination IDs from the appended pages",async()=>{
 await open();vi.mocked(nativeAdapter.openPdf).mockResolvedValueOnce({id:"other",name:"Other.pdf",pages:[{width:300,height:400}],bookmarks:[{id:"a",title:"Appended",level:0,destination:fit("0")}]});
 fireEvent.click(screen.getByRole("button",{name:"Add PDF"}));await screen.findByRole("treeitem",{name:/^Appended/});fireEvent.click(screen.getByRole("treeitem",{name:/^Appended/}));expect(screen.getByText("Page 4 of 4")).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalled());const [pages,options]=vi.mocked(nativeAdapter.exportPdf).mock.calls[0];expect(bookmarkPage(options!.bookmarks![3])).toBe(pages[3].id);expect(new Set(options!.bookmarks!.map(b=>b.id)).size).toBe(4);
});
it("requires explicit discard of unreadable bookmarks and supports undoing that decision",async()=>{
 vi.mocked(nativeAdapter.openPdf).mockResolvedValueOnce({id:"source",name:"Book.pdf",pages:[{width:300,height:400}],bookmarkError:"Cyclic outline tree"});await open();
 expect(screen.getByText(/Cyclic outline tree/)).toBeInTheDocument();fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(screen.getByRole("alert")).toHaveTextContent(/bookmark/i));expect(nativeAdapter.exportPdf).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"Discard unreadable bookmarks"}));fireEvent.click(screen.getByRole("button",{name:"Save a copy"}));await waitFor(()=>expect(nativeAdapter.exportPdf).toHaveBeenCalled());expect(vi.mocked(nativeAdapter.exportPdf).mock.calls[0][1]).toEqual({bookmarks:[]});
 fireEvent.click(screen.getByRole("button",{name:"Undo"}));expect(screen.getByText(/Cyclic outline tree/)).toBeInTheDocument();
});
it("supports keyboard tree navigation and validated page jumps",async()=>{
 await open();const chapter=screen.getByRole("treeitem",{name:/^Chapter/});chapter.focus();fireEvent.keyDown(chapter,{key:"ArrowDown"});expect(screen.getByRole("treeitem",{name:/^Section/})).toHaveFocus();
 fireEvent.keyDown(document.activeElement!,{key:"Enter"});expect(screen.getByText("Page 3 of 3")).toBeInTheDocument();
 fireEvent.click(screen.getByRole("button",{name:"Go to page"}));fireEvent.change(screen.getByRole("spinbutton",{name:"Page number"}),{target:{value:"0"}});fireEvent.click(screen.getByRole("button",{name:"Go"}));expect(within(screen.getByRole("dialog")).getByRole("alert")).toBeInTheDocument();
 fireEvent.change(screen.getByRole("spinbutton",{name:"Page number"}),{target:{value:"1"}});fireEvent.click(screen.getByRole("button",{name:"Go"}));expect(screen.getByText("Page 1 of 3")).toBeInTheDocument();expect(screen.queryByLabelText("Unsaved changes")).not.toBeInTheDocument();
});
it("bookmark navigation wins over active Find results",async()=>{
 vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,"getPageText").mockResolvedValue({characters:Array.from("Needle",(text,i)=>({text,x:i*6,y:30,width:6,height:12}))});await open();
 fireEvent.click(screen.getByRole("button",{name:"Find"}));fireEvent.change(screen.getByRole("searchbox"),{target:{value:"Needle"}});await screen.findByText("1 of 3");
 fireEvent.click(screen.getByRole("treeitem",{name:/^Section/}));await act(async()=>{await new Promise(resolve=>setTimeout(resolve,250));});expect(screen.getByText("Page 3 of 3")).toBeInTheDocument();expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
});
it("keeps keyboard focus on a visible row after deleting the last root behind a collapsed branch",async()=>{
 await open();fireEvent.click(screen.getByRole("button",{name:"Collapse Chapter"}));
 fireEvent.click(screen.getByRole("treeitem",{name:/^Opening/}));fireEvent.click(screen.getByRole("button",{name:"Delete bookmark"}));
 const chapter=screen.getByRole("treeitem",{name:/^Chapter/});expect(chapter).toHaveFocus();expect(chapter).toHaveAttribute("tabindex","0");expect(chapter).toHaveAttribute("aria-selected","true");
 fireEvent.keyDown(chapter,{key:"ArrowRight"});fireEvent.keyDown(chapter,{key:"ArrowDown"});expect(screen.getByRole("treeitem",{name:/^Section/})).toHaveFocus();
});
