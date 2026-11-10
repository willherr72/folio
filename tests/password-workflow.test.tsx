import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {PasswordDialog} from "../src/components/PasswordDialog";
import {usePasswordWorkflow} from "../src/editor/usePasswordWorkflow";
import {openProtectedPdf} from "../src/editor/password-native";
import type {FolioAdapter} from "../src/editor/adapter";
import {invoke} from "@tauri-apps/api/core";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();});
const challenge={status:"passwordRequired" as const,challengeId:"challenge",name:"Locked.pdf",incorrect:false};
const document={id:"opened",name:"Locked.pdf",pages:[{width:300,height:400}]};
function adapter():FolioAdapter {return {kind:"native",openPdf:options=>openProtectedPdf(options!),renderPage:vi.fn(),exportPdf:vi.fn(),engineStatus:async()=>"test",closeDocument:id=>invoke("close_document",{sourceId:id})};}
function Harness({api,onOpen=vi.fn()}:{api:FolioAdapter;onOpen?:(value:unknown)=>void}) {
 const passwords=usePasswordWorkflow();
 return <><button onClick={()=>void passwords.open(api).then(onOpen)}>Open</button>{passwords.prompt&&<PasswordDialog {...passwords.prompt} onSubmit={passwords.submit} onCancel={passwords.cancel}/>}</>;
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
});
it("unmounting while the picker opens cancels a late challenge",async()=>{
 let finish!:(value:unknown)=>void;vi.mocked(invoke).mockImplementation(command=>command==="begin_open_pdf"?new Promise(resolve=>{finish=resolve;}):Promise.resolve(undefined));
 const view=render(<Harness api={adapter()}/>);fireEvent.click(screen.getByText("Open"));view.unmount();
 await act(async()=>finish(challenge));
 expect(invoke).toHaveBeenCalledWith("cancel_open_pdf",{challengeId:"challenge"});
 expect(invoke).not.toHaveBeenCalledWith("submit_open_password",expect.anything());
});
it("allows an empty password and does not trim spaces or Unicode",async()=>{
 const submit=vi.fn();render(<PasswordDialog mode="open" name="Protected.pdf" busy={false} error={null} onSubmit={submit} onCancel={vi.fn()}/>);
 const input=screen.getByLabelText("PDF password");fireEvent.submit(input.closest("form")!);expect(submit).toHaveBeenCalledWith("");
 fireEvent.change(input,{target:{value:" café-读者 "}});fireEvent.submit(input.closest("form")!);expect(submit).toHaveBeenLastCalledWith(" café-读者 ");
});
