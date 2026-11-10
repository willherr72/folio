import {useState} from "react";
import {Modal} from "./Modal";
import type {DocumentBookmark,PagePlan} from "../editor/types";
import "./release-dialogs.css";
export function BookmarkDialog({bookmark,pages,currentPageId,onSave,onClose}:{bookmark?:DocumentBookmark;pages:PagePlan[];currentPageId:string|null;onSave(title:string,pageId?:string):void;onClose():void}){
 const [title,setTitle]=useState(bookmark?.title??"");
 const [target,setTarget]=useState(bookmark?"unchanged":String(Math.max(0,pages.findIndex(p=>p.id===currentPageId))));
 const [error,setError]=useState<string|null>(null);
 const save=()=>{try{onSave(title,target==="unchanged"?undefined:pages[Number(target)]?.id);}catch(e){setError(e instanceof Error?e.message:String(e));}};
 return <Modal title={bookmark?"Edit bookmark":"Add bookmark"} className="release-dialog" onClose={onClose}>
  <form onSubmit={e=>{e.preventDefault();save();}}>
   <div className="release-form bookmark-form">
    <label>Title<input autoFocus value={title} onChange={e=>setTitle(e.target.value)} /></label>
    <label>Destination<select value={target} onChange={e=>setTarget(e.target.value)}>{bookmark&&<option value="unchanged">Keep existing destination</option>}{pages.map((p,i)=><option key={p.id} value={i}>Page {i+1}</option>)}</select></label>
    {error&&<p className="error-note" role="alert">{error}</p>}
   </div>
   <div className="dialog-actions"><button className="button" type="button" onClick={onClose}>Cancel</button><button className="button primary" type="submit">{bookmark?"Save bookmark":"Add bookmark"}</button></div>
  </form>
 </Modal>;
}
