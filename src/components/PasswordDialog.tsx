import {useEffect,useId,useRef,useState} from "react";
import {Modal} from "./Modal";
import type {PasswordPrompt} from "../editor/usePasswordWorkflow";
import "./release-dialogs.css";

export function PasswordDialog({mode,name,busy,error,onSubmit,onCancel}:PasswordPrompt&{onSubmit(password:string):void;onCancel():void}) {
 const [password,setPassword]=useState("");const input=useRef<HTMLInputElement>(null);const errorId=useId();
 useEffect(()=>{if(!busy)input.current?.focus();},[busy,error]);
 const tooLong=new TextEncoder().encode(password).length>1024;
 const submit=(event:React.FormEvent)=>{event.preventDefault();if(busy||tooLong)return;const value=password;setPassword("");onSubmit(value);};
 const message=tooLong?"Use a password of at most 1,024 bytes.":error;
 return <Modal className="release-dialog password-dialog" title={mode==="owner"?"Unlock editing":"Open protected PDF"} description={mode==="owner"?`Enter the owner password for ${name}. Saved copies keep the original passwords and permissions.`:`Enter the password for ${name}.`} onClose={onCancel} initialFocusRef={input}>
  <form onSubmit={submit}>
   <div className="release-form"><label>{mode==="owner"?"Owner password":"PDF password"}<input ref={input} type="password" autoComplete="off" spellCheck={false} disabled={busy} value={password} aria-invalid={!!message} aria-describedby={message?errorId:undefined} onChange={event=>setPassword(event.target.value)}/></label>
   {message&&<p id={errorId} className="error-note" role="alert">{message}</p>}{busy&&<p role="status">Checking password…</p>}</div>
   <div className="dialog-actions"><button type="button" className="button" onClick={()=>{setPassword("");onCancel();}}>Cancel</button><button type="submit" className="button primary" disabled={busy||tooLong}>{mode==="owner"?"Unlock editing":"Open PDF"}</button></div>
  </form>
 </Modal>;
}
