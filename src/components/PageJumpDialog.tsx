import {useState} from "react";
import {Modal} from "./Modal";
import "./release-dialogs.css";
export function PageJumpDialog({current,total,onGo,onClose}:{current:number;total:number;onGo(index:number):void;onClose():void}){
 const [value,setValue]=useState(String(current+1)),[error,setError]=useState(false);
 const go=()=>{const n=Number(value);if(!/^[1-9]\d*$/.test(value)||!Number.isSafeInteger(n)||n>total){setError(true);return;}onGo(n-1);};
 return <Modal title="Go to page" className="release-dialog" onClose={onClose}>
  <form noValidate onSubmit={e=>{e.preventDefault();go();}}><div className="release-form bookmark-form"><label>Page number<input autoFocus type="number" min={1} max={total} value={value} onChange={e=>{setValue(e.target.value);setError(false);}} aria-invalid={error}/></label>{error&&<p role="alert" className="error-note">Enter a page number from 1 to {total}.</p>}</div><div className="dialog-actions"><button className="button" type="button" onClick={onClose}>Cancel</button><button className="button primary" type="submit">Go</button></div></form>
 </Modal>;
}
