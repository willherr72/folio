import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {PasswordDialog} from "../src/components/PasswordDialog";
import {usePasswordWorkflow} from "../src/editor/usePasswordWorkflow";
import {openProtectedPdf} from "../src/editor/password-native";
import type {FolioAdapter} from "../src/editor/adapter";
import type {DocumentInfo} from "../src/editor/types";
import {invoke} from "@tauri-apps/api/core";
import {holdCustomFont} from "../src/editor/custom-fonts";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const challenge={status:"passwordRequired" as const,challengeId:"challenge",name:"Locked.pdf",incorrect:false};
const document={id:"opened",name:"Locked.pdf",pages:[{width:300,height:400}]};
function adapter():FolioAdapter {return {kind:"native",openPdf:options=>openProtectedPdf(options!),renderPage:vi.fn(),exportPdf:vi.fn(),engineStatus:async()=>"test",closeDocument:id=>invoke("close_document",{sourceId:id})};}
function Harness({api,onOpen=vi.fn(),prepare}:{api:FolioAdapter;onOpen?:(value:unknown)=>void;prepare?:()=>Promise<void>}) {
 const passwords=usePasswordWorkflow();
 return <><button onClick={()=>void passwords.open(api,prepare).then(onOpen)}>Open</button><button onClick={passwords.cancel}>Abort pending work</button><output data-testid="opening">{String(passwords.opening)}</output>{passwords.prompt&&<PasswordDialog {...passwords.prompt} onSubmit={passwords.submit} onCancel={passwords.cancel}/>}</>;
}
it("masks input, clears each submitted password, retries and installs only successful results",async()=>{
 const opened=vi.fn();vi.mocked(invoke).mockImplementation(async(command)=>command==="begin_open_pdf"?challenge:command==="submit_open_password"?{...challenge,incorrect:true}:undefined);
 render(<Harness api={adapter()} onOpen={opened}/>);fireEvent.click(screen.getByText("Open"));
 const input=await screen.findByLabelText("PDF password");expect(input).toHaveAttribute("type","password");expect(input).toHaveFocus();
 fireEvent.change(input,{target:{value:"wrong"}});fireEvent.click(screen.getByRole("button",{name:"Open PDF"}));
 await screen.findByRole("alert");expect(input).toHaveValue("");expect(opened).not.toHaveBeenCalled();
 vi.mocked(invoke).mockResolvedValueOnce({status:"opened",document});
 fireEvent.change(input,{target:{value:"reader"}});fireEvent.submit(input.closest("form")!);
 await waitFor(()=>expect(opened).toHaveBeenCalledWith(document));expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
 expect(invoke).toHaveBeenCalledWith("submit_open_password",{challengeId:"challenge",password:"reader"});
});
it("cancels during verification and releases a late source instead of opening it",async()=>{
 let finish!:(value:unknown)=>void;const opened=vi.fn();
 vi.mocked(invoke).mockImplementation(command=>command==="begin_open_pdf"?Promise.resolve(challenge):command==="submit_open_password"?new Promise(resolve=>{finish=resolve;}):Promise.resolve(undefined));
 render(<Harness api={adapter()} onOpen={opened}/>);fireEvent.click(screen.getByText("Open"));
 const input=await screen.findByLabelText("PDF password");fireEvent.change(input,{target:{value:"reader"}});fireEvent.submit(input.closest("form")!);
 await screen.findByText("Checking password…");fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
 const fontId="a".repeat(64);
 await act(async()=>finish({status:"opened",document:{...document,pages:[{width:300,height:400,overlays:[{type:"text",id:"t",fontId,text:"note",x:10,y:20,fontSize:12,color:"#000000"}]}]}}));
 await waitFor(()=>expect(opened).toHaveBeenCalledWith(null));
 expect(invoke).toHaveBeenCalledWith("cancel_open_pdf",{challengeId:"challenge"});
 expect(invoke).toHaveBeenCalledWith("close_document",{sourceId:"opened"});
 expect(invoke).toHaveBeenCalledWith("release_font",{id:fontId});
 expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="close_document")).toHaveLength(1);
 expect(vi.mocked(invoke).mock.calls.filter(([command])=>command==="release_font")).toHaveLength(1);
});
it("settles cancellation before a pending password verification replies",async()=>{
 let finish!:(value:unknown)=>void;const opened=vi.fn();
 vi.mocked(invoke).mockImplementation(command=>command==="begin_open_pdf"?Promise.resolve(challenge):command==="submit_open_password"?new Promise(resolve=>{finish=resolve;}):Promise.resolve(undefined));
 render(<Harness api={adapter()} onOpen={opened}/>);fireEvent.click(screen.getByText("Open"));
 const input=await screen.findByLabelText("PDF password");fireEvent.change(input,{target:{value:"reader"}});fireEvent.submit(input.closest("form")!);
 await screen.findByText("Checking password…");fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
 try {await waitFor(()=>expect(opened).toHaveBeenCalledWith(null));}
 finally {await act(async()=>finish({status:"opened",document}));}
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith("close_document",{sourceId:"opened"}));
 expect(opened).toHaveBeenCalledTimes(1);
});
it("unmounting while the picker opens cancels a late challenge",async()=>{
 let finish!:(value:unknown)=>void;vi.mocked(invoke).mockImplementation(command=>command==="begin_open_pdf"?new Promise(resolve=>{finish=resolve;}):Promise.resolve(undefined));
 const view=render(<Harness api={adapter()}/>);fireEvent.click(screen.getByText("Open"));view.unmount();
 await act(async()=>finish(challenge));
 expect(invoke).toHaveBeenCalledWith("cancel_open_pdf",{challengeId:"challenge"});
 expect(invoke).not.toHaveBeenCalledWith("submit_open_password",expect.anything());
});
it("cancels resource preparation without launching a later picker",async()=>{
 let finish!:()=>void;const prepare=vi.fn(()=>new Promise<void>(resolve=>{finish=resolve;})),opened=vi.fn(),api=adapter();
 const pick=vi.spyOn(api,"openPdf");render(<Harness api={api} prepare={prepare} onOpen={opened}/>);
 fireEvent.click(screen.getByText("Open"));expect(screen.getByTestId("opening")).toHaveTextContent("true");
 fireEvent.click(screen.getByText("Abort pending work"));
 try {await waitFor(()=>expect(opened).toHaveBeenCalledWith(null));}
 finally {await act(async()=>finish());}
 expect(pick).not.toHaveBeenCalled();expect(screen.getByTestId("opening")).toHaveTextContent("false");
});
it("keeps a new password prompt when an ignored cancelled open returns late",async()=>{
 let finish!:(value:DocumentInfo|null)=>void;const api=adapter(),opened=vi.fn();vi.mocked(invoke).mockResolvedValue(undefined);
 vi.spyOn(api,"openPdf").mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;})).mockImplementationOnce(async options=>{
  await options!.requestPassword({...challenge,name:"New protected.pdf"});return {...document,id:"second"};
 });
 render(<Harness api={api} onOpen={opened}/>);fireEvent.click(screen.getByText("Open"));fireEvent.click(screen.getByText("Abort pending work"));
 try {
  await waitFor(()=>expect(opened).toHaveBeenCalledWith(null));fireEvent.click(screen.getByText("Open"));
  await screen.findByText("Enter the password for New protected.pdf.");
 } finally {await act(async()=>finish({...document,id:"first-late"}));}
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith("close_document",{sourceId:"first-late"}));
 expect(screen.getByRole("dialog")).toBeInTheDocument();expect(screen.getByTestId("opening")).toHaveTextContent("true");
 fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
 await waitFor(()=>expect(opened).toHaveBeenCalledTimes(2));expect(opened.mock.calls.every(([value])=>value===null)).toBe(true);
});
it("does not release a leased font while discarding an ignored cancelled open",async()=>{
 let finish!:(value:DocumentInfo|null)=>void;const api=adapter(),opened=vi.fn(),fontId="c".repeat(64),release=holdCustomFont(fontId);
 vi.mocked(invoke).mockResolvedValue(undefined);vi.spyOn(api,"openPdf").mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
 render(<Harness api={api} onOpen={opened}/>);fireEvent.click(screen.getByText("Open"));fireEvent.click(screen.getByText("Abort pending work"));
 try {
  await waitFor(()=>expect(opened).toHaveBeenCalledWith(null));
  await act(async()=>finish({...document,id:"font-late",pages:[{width:300,height:400,overlays:[{type:"text",id:"note",x:10,y:10,text:"note",fontSize:12,fontId,color:"#000000"}]}]}));
  await waitFor(()=>expect(invoke).toHaveBeenCalledWith("close_document",{sourceId:"font-late"}));
  expect(invoke).not.toHaveBeenCalledWith("release_font",{id:fontId});
 }finally{await release();}
 expect(invoke).toHaveBeenCalledWith("release_font",{id:fontId});
});
it("allows an empty password and does not trim spaces or Unicode",async()=>{
 const submit=vi.fn();render(<PasswordDialog mode="open" name="Protected.pdf" busy={false} error={null} onSubmit={submit} onCancel={vi.fn()}/>);
 const input=screen.getByLabelText("PDF password");fireEvent.submit(input.closest("form")!);expect(submit).toHaveBeenCalledWith("");
 fireEvent.change(input,{target:{value:" café-读者 "}});fireEvent.submit(input.closest("form")!);expect(submit).toHaveBeenLastCalledWith(" café-读者 ");
});
