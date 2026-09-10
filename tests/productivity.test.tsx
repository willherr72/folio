import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../src/App";
import { nativeAdapter } from "../src/editor/adapter";
beforeEach(()=>{localStorage.clear(); vi.stubGlobal("PointerEvent", MouseEvent);});
afterEach(()=>{cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals();});
async function demo() { render(<App initialDemo={false}/>); fireEvent.click(screen.getByRole("button",{name:"Explore demo"})); await screen.findByText("Folio welcome.pdf"); }
describe("productivity",()=>{
  it("focuses and selects the new text placeholder immediately",async()=>{
    await demo();
    fireEvent.click(screen.getByRole("button",{name:"Text"}));
    const canvas=document.querySelector(".page-canvas")!;
    vi.spyOn(canvas,"getBoundingClientRect").mockReturnValue({left:0,top:0,width:612,height:792,right:612,bottom:792,x:0,y:0,toJSON(){}});
    fireEvent.pointerDown(canvas,{button:0,clientX:100,clientY:200});
    const content=await screen.findByRole("textbox",{name:"Content"}) as HTMLTextAreaElement;
    expect(content).toHaveFocus();
    expect(content.selectionStart).toBe(0);
    expect(content.selectionEnd).toBe(content.value.length);
    fireEvent.change(content,{target:{value:"Ready to type"}});
    expect(content.value).toBe("Ready to type");
  });
  it("opens a new tab without discarding edits and restores independent zoom/history",async()=>{
    vi.spyOn(nativeAdapter,"openPdf").mockResolvedValue({id:"second",name:"Second.pdf",pages:[{width:300,height:400}]});
    vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");
    await demo();
    fireEvent.click(screen.getByRole("button",{name:"Rotate"}));
    fireEvent.click(screen.getByRole("button",{name:"Zoom in"}));
    fireEvent.click(screen.getByRole("button",{name:"Open"}));
    await screen.findByRole("tab",{name:"Second.pdf"});
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button",{name:"90%"})).toBeInTheDocument();
    expect(screen.getByRole("button",{name:"Undo"})).toBeDisabled();
    fireEvent.click(screen.getByRole("tab",{name:"Folio welcome.pdf"}));
    expect(screen.getByText("90° clockwise")).toBeInTheDocument();
    expect(screen.getByRole("button",{name:"100%"})).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button",{name:"Close Folio welcome.pdf"}));
    expect(await screen.findByRole("dialog",{name:"Close document?"})).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button",{name:"Keep editing"}));
    expect(screen.getAllByRole("tab")).toHaveLength(2);
  });
  it("releases only a closed tab's sources and keeps cancelled picker edits",async()=>{
    const open=vi.spyOn(nativeAdapter,"openPdf").mockResolvedValueOnce({id:"a",name:"A.pdf",pages:[{width:300,height:400}]}).mockResolvedValueOnce({id:"b",name:"B.pdf",pages:[{width:300,height:400}]}).mockResolvedValue(null);
    vi.spyOn(nativeAdapter,"renderPage").mockResolvedValue("data:image/png;base64,");
    const close=vi.spyOn(nativeAdapter,"closeDocument").mockResolvedValue();
    render(<App initialDemo={false}/>);
    fireEvent.click(screen.getByRole("button",{name:"Open a PDF"}));
    await screen.findByRole("tab",{name:"A.pdf"});
    fireEvent.click(screen.getByRole("button",{name:"Open"}));
    await screen.findByRole("tab",{name:"B.pdf"});
    fireEvent.click(screen.getByRole("button",{name:"Rotate"}));
    fireEvent.click(screen.getByRole("button",{name:"Open"}));
    await waitFor(()=>expect(open).toHaveBeenCalledTimes(3));
    expect(screen.getByText("90° clockwise")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button",{name:"Close A.pdf"}));
    await waitFor(()=>expect(close).toHaveBeenCalledWith("a"));
    expect(close).not.toHaveBeenCalledWith("b");
    expect(screen.getByRole("tab",{name:"B.pdf"})).toHaveAttribute("aria-selected","true");
  });
});
