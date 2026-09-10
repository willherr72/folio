import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,describe,expect,it,vi} from "vitest";
import {SignaturePad} from "../src/components/SignaturePad";
import {loadSignatures,saveSignature,SIGNATURES_KEY} from "../src/editor/signatures";
const paths=[[{x:12,y:45},{x:88,y:15},{x:132,y:70}]];
afterEach(()=>{cleanup();vi.restoreAllMocks();localStorage.clear();});
describe("signature library dialog",()=>{
 it("reuses a saved drawing without changing its paths and supports rename/delete",()=>{
  saveSignature("Everyday",paths);
  const accept=vi.fn();const view=render(<SignaturePad onCancel={()=>{}} onAccept={accept}/>);
  fireEvent.click(screen.getByRole("button",{name:"Select signature Everyday"}));
  expect(screen.getByRole("img",{name:"Selected signature preview"})).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button",{name:"Use signature"}));
  expect(accept).toHaveBeenCalledWith(paths);
  fireEvent.change(screen.getByLabelText("Signature name"),{target:{value:"Full name"}});
  fireEvent.click(screen.getByRole("button",{name:"Rename"}));
  expect(loadSignatures()[0].name).toBe("Full name");
  view.unmount();render(<SignaturePad onCancel={()=>{}} onAccept={accept}/>);
  fireEvent.click(screen.getByRole("button",{name:"Select signature Full name"}));
  fireEvent.click(screen.getByRole("button",{name:"Delete saved signature"}));
  expect(loadSignatures()).toEqual([]);
  expect(screen.getByRole("button",{name:"Use signature"})).toBeDisabled();
 });
 it("shows storage errors without pretending a rename succeeded",()=>{
  saveSignature("Kept",paths);render(<SignaturePad onCancel={()=>{}} onAccept={()=>{}}/>);
  fireEvent.click(screen.getByRole("button",{name:"Select signature Kept"}));
  vi.spyOn(Storage.prototype,"setItem").mockImplementation(()=>{throw new Error("disk full");});
  fireEvent.change(screen.getByLabelText("Signature name"),{target:{value:"Changed"}});
  fireEvent.click(screen.getByRole("button",{name:"Rename"}));
  expect(screen.getByRole("alert")).toHaveTextContent(/could not be saved/i);
  expect(loadSignatures()[0].name).toBe("Kept");
 });
 it("keeps drawing available when saved data is damaged and requires an explicit reset",()=>{
  localStorage.setItem(SIGNATURES_KEY,"broken");render(<SignaturePad onCancel={()=>{}} onAccept={()=>{}}/>);
  expect(screen.getByRole("alert")).toHaveTextContent(/damaged/i);
  expect(screen.getByLabelText("Signature drawing area")).toBeInTheDocument();
  expect(screen.getByText(/visual signatures.*certificate/i)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button",{name:"Reset saved library"}));
  expect(screen.getByText("Remove every saved signature from this device? This cannot be undone.")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button",{name:"Remove saved library"}));
  expect(loadSignatures()).toEqual([]);
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
 });
});

