import {useRef,useState} from "react";
import {Modal} from "./Modal";
import "./release-dialogs.css";
export type BlankPagePosition="before"|"after"|"end";
export interface BlankPageOptions {width:number;height:number;position:BlankPagePosition}
const sizes={letter:[612,792],a4:[595.28,841.89],legal:[612,1008]} as const;
export function BlankPageDialog({inserting,onCreate,onClose}:{inserting:boolean;onCreate(options:BlankPageOptions):Promise<boolean>;onClose():void}){
 const [paper,setPaper]=useState<keyof typeof sizes>("letter");
 const [orientation,setOrientation]=useState("portrait");
 const [position,setPosition]=useState<BlankPagePosition>("after");
 const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null);
 const pending=useRef(false);
 const [short,long]=sizes[paper];const width=orientation==="portrait"?short:long,height=orientation==="portrait"?long:short;
 const close=()=>{if(!pending.current)onClose();};
 const create=async()=>{
  if(pending.current)return;
  pending.current=true;setBusy(true);setError(null);
  try{if(await onCreate({width,height,position}))onClose();}
  catch(error){setError(error instanceof Error?error.message:String(error));}
  finally{pending.current=false;setBusy(false);}
 };
 return <Modal className="release-dialog" title={inserting?"Insert blank page":"New PDF"} description={inserting?"Add a blank page to this document. You can undo the insertion.":"Start an empty PDF and add text, drawings, or other pages."} onClose={close}>
  <div className="release-form">
   <label>Paper size<select disabled={busy} value={paper} onChange={event=>setPaper(event.target.value as typeof paper)}><option value="letter">Letter · 8.5 × 11 in</option><option value="a4">A4 · 210 × 297 mm</option><option value="legal">Legal · 8.5 × 14 in</option></select></label>
   <label>Orientation<select disabled={busy} value={orientation} onChange={event=>setOrientation(event.target.value)}><option value="portrait">Portrait</option><option value="landscape">Landscape</option></select></label>
   {inserting&&<label>Insert<select disabled={busy} value={position} onChange={event=>setPosition(event.target.value as BlankPagePosition)}><option value="before">Before current page</option><option value="after">After current page</option><option value="end">At end of document</option></select></label>}
   <p>{width} × {height} pt</p>
   {error&&<p className="error-note" role="alert">{error}</p>}
   {busy&&<p role="status">Creating page…</p>}
  </div>
  <div className="dialog-actions"><button className="button" disabled={busy} onClick={close}>Cancel</button><button className="button primary" disabled={busy} onClick={()=>void create()}>{inserting?"Insert page":"Create PDF"}</button></div>
 </Modal>;
}
