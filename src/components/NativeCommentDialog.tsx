import {useEffect,useRef,useState} from "react";
import "./release-dialogs.css";
import {Modal} from "./Modal";
import type {NativeComment,NativeCommentChanges,PagePlan} from "../editor/types";
import {displayDimensions} from "../editor/geometry";
import {displayNativeCommentBounds,nativeCommentPosition} from "../editor/native-comments";

export function NativeCommentDialog({comment,page,pageNumber,reason,onApply,onDelete,onClose}:{comment:NativeComment;page:PagePlan;pageNumber:number;reason?:string|null;onApply(changes:NativeCommentChanges):Promise<void>;onDelete?():Promise<void>;onClose():void}){
 const bounds=displayNativeCommentBounds(comment.bounds,page),size=displayDimensions(page.width,page.height,page.rotation);
 const [text,setText]=useState(comment.text),[left,setLeft]=useState(String(bounds.x)),[top,setTop]=useState(String(bounds.y)),[color,setColor]=useState(comment.color??"#ffcc00");
 const [colorEdited,setColorEdited]=useState(false);
 const [busy,setBusy]=useState(false),[error,setError]=useState<string|null>(null);
 const pending=useRef(false),alive=useRef(true);
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
 const positionChanged=left!==String(bounds.x)||top!==String(bounds.y),colorChanged=!!comment.canChangeColor&&colorEdited&&color!==comment.color;
 const changed=text!==comment.text||positionChanged||colorChanged;
 const x=Number(left),y=Number(top);
 const validPosition=!positionChanged||(left.trim()!==""&&top.trim()!==""&&Number.isFinite(x)&&Number.isFinite(y)&&x>=0&&y>=0&&x+bounds.width<=size.width+0.01&&y+bounds.height<=size.height+0.01);
 const close=()=>{if(!pending.current)onClose();};
 const run=async(action:()=>Promise<void>)=>{
  if(pending.current||reason)return;
  pending.current=true;setBusy(true);setError(null);
  try{await action();}catch(value){if(alive.current)setError(value instanceof Error?value.message:String(value));}
  finally{pending.current=false;if(alive.current)setBusy(false);}
 };
 const apply=()=>{
  if(!changed||!validPosition)return;
  const changes:NativeCommentChanges={text};
  if(positionChanged)changes.position=nativeCommentPosition({...bounds,x,y},page);
  if(colorChanged)changes.color=color;
  void run(()=>onApply(changes));
 };
 const remove=()=>{if(onDelete&&!comment.deleteReason)void run(onDelete);};
 return <Modal className="release-dialog" title="PDF comment" description={`Page ${pageNumber}. The original comment icon and popup are preserved.`} onClose={close}>
  <div className="release-form">
  {comment.author&&<p>Author: {comment.author}</p>}{comment.subject&&<p>Subject: {comment.subject}</p>}
  <label className="native-comment-text">Comment text<textarea rows={8} value={text} readOnly={!!reason} disabled={busy} onChange={event=>{setText(event.target.value);setError(null);}}/></label>
  <div className="native-comment-position">
   <label>Left (pt)<input type="number" step="any" min={0} max={Math.max(0,size.width-bounds.width)} value={left} disabled={busy||!!reason} onChange={event=>setLeft(event.target.value)}/></label>
   <label>Top (pt)<input type="number" step="any" min={0} max={Math.max(0,size.height-bounds.height)} value={top} disabled={busy||!!reason} onChange={event=>setTop(event.target.value)}/></label>
   <label>Comment color<input type="color" value={color} disabled={busy||!!reason||!comment.canChangeColor} onChange={event=>{setColor(event.target.value);setColorEdited(true);}}/></label>
  </div>
  {!validPosition&&<p>Place the entire comment icon inside the page.</p>}
  {comment.canChangeColor===false&&<p>This icon has a custom appearance. Its original color is preserved.</p>}
  {reason&&<p>{reason}</p>}{!reason&&comment.deleteReason&&<p>{comment.deleteReason}</p>}
  {error&&<p role="alert" className="error-note">{error}</p>}{busy&&<p role="status">Updating comment…</p>}
  </div>
  <div className="dialog-actions">
   {onDelete&&<button className="button danger" disabled={busy||!!reason||!!comment.deleteReason} onClick={remove}>Delete comment</button>}
   <button className="button" disabled={busy} onClick={close}>{changed?"Discard draft":"Close"}</button>
   <button className="button primary" disabled={busy||!!reason||!changed||!validPosition} onClick={apply}>Apply comment</button>
  </div>
 </Modal>;
}
