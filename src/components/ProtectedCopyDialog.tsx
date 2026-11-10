import {useEffect,useRef,useState} from 'react';
import {Modal} from './Modal';
import type {FolioAdapter} from '../editor/adapter';
import type {DocumentBookmark,PagePlan} from '../editor/types';
import type {PdfSaveProgress,PdfSaveTask} from '../editor/pdf-save';
import {defaultPdfPermissions,protectionChangeDenied,type ProtectionChange} from '../editor/protected-copy';
import './release-dialogs.css';
interface Props{pages:PagePlan[];bookmarks?:DocumentBookmark[];metadata?:import("../editor/document-properties").DocumentMetadataEdit;adapter:FolioAdapter;onClose():void;onComplete(path:string):void;onBusyChange(busy:boolean):void}
export function ProtectedCopyDialog({pages,bookmarks,metadata,adapter,onClose,onComplete,onBusyChange}:Props){
 const [mode,setMode]=useState<'passwords'|'remove'>('passwords'),[user,setUser]=useState(''),[userConfirm,setUserConfirm]=useState(''),[owner,setOwner]=useState(''),[ownerConfirm,setOwnerConfirm]=useState('');
 const [permissions,setPermissions]=useState(defaultPdfPermissions),[busy,setBusy]=useState(false),[cancelling,setCancelling]=useState(false),[finishing,setFinishing]=useState(false),[failure,setFailure]=useState<string|null>(null),[notice,setNotice]=useState<string|null>(null),[progress,setProgress]=useState<PdfSaveProgress|null>(null);
 const pending=useRef(false),alive=useRef(true),task=useRef<PdfSaveTask|null>(null),cancelPending=useRef<PdfSaveTask|null>(null),notifyBusy=useRef(onBusyChange);notifyBusy.current=onBusyChange;
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;void task.current?.cancel().catch(()=>{});};},[]);
 const clearPasswords=()=>{setUser('');setUserConfirm('');setOwner('');setOwnerConfirm('');};
 const denied=protectionChangeDenied(pages);
 const invalid=mode==='passwords'&&(!owner||user!==userConfirm||owner!==ownerConfirm||user===owner||[user,owner,userConfirm,ownerConfirm].some(value=>new TextEncoder().encode(value).length>1024));
 const cancel=async()=>{
  if(!pending.current){clearPasswords();onClose();return;}
  const work=task.current;if(!work||cancelPending.current===work||finishing)return;
  cancelPending.current=work;setCancelling(true);setFailure(null);
  try{const accepted=await work.cancel();if(alive.current&&task.current===work&&!accepted){setCancelling(false);setFinishing(true);}}
  catch(error){if(alive.current&&task.current===work){setFailure(`Could not request cancellation: ${error instanceof Error?error.message:String(error)}`);setCancelling(false);}}
  finally{if(cancelPending.current===work)cancelPending.current=null;}
 };
 const start=async()=>{
  if(pending.current||invalid||denied||!adapter.startPdfSave)return;
  const security:ProtectionChange=mode==='remove'?{mode:'remove'}:{mode:'passwords',userPassword:user,ownerPassword:owner,permissions};
  pending.current=true;notifyBusy.current(true);setBusy(true);setFailure(null);setNotice(null);setProgress(null);setFinishing(false);setCancelling(false);
  try{
   const work=adapter.startPdfSave(pages,{security,...(metadata!==undefined?{metadata}:{}),...(bookmarks!==undefined?{bookmarks}:{})},event=>{if(alive.current){setProgress(event);if(event.stage==='finishing')setFinishing(true);}});task.current=work;
   const output=await work.result;if(alive.current){if(output)onComplete(output);else setNotice('Save cancelled. The current document is unchanged.');}
  }catch(error){if(alive.current)setFailure(error instanceof Error?error.message:String(error));}
  finally{task.current=null;pending.current=false;notifyBusy.current(false);if(alive.current){clearPasswords();setBusy(false);setCancelling(false);setFinishing(false);}}
 };
 const stage=progress?.stage;
 const status=finishing?'Finishing save…':cancelling?'Cancelling… the current operation may need to finish first.':stage==='choosingFile'?'Choose a destination in the Save dialog. Cancel that dialog to return.':stage==='securing'?'Applying security settings…':stage==='writing'?'Writing the PDF copy…':progress?`Preparing PDF copy: ${progress.completed} of ${progress.total} pages.`:'Starting save…';
 const checkbox=(key:'copyContent'|'editContent'|'annotate'|'fillForms'|'assemble',label:string)=><label style={{display:'flex',alignItems:'center',gap:8}}><input type="checkbox" disabled={busy||(key==='fillForms'&&permissions.annotate)} checked={permissions[key]} onChange={event=>{const checked=event.target.checked;setPermissions(value=>({...value,[key]:checked,...(key==='annotate'&&checked?{fillForms:true}:{})}));}}/>{label}</label>;
 return <Modal className="release-dialog" title="Security settings" description="Save your edited PDF as a copy with new security settings. The current document keeps its existing security and unsaved changes." onClose={()=>void cancel()}>
  <div className="release-form">
   <label>Security<select disabled={busy} value={mode} onChange={event=>{setMode(event.target.value as typeof mode);clearPasswords();setFailure(null);}}><option value="passwords">Password protection</option><option value="remove">No password protection</option></select></label>
   {mode==='passwords'?<>
    <p>AES-256 encryption. Leave the opening password empty to allow opening without a password. Keep your passwords safe; Folio cannot recover them.</p>
    <label>Opening password<input type="password" autoComplete="new-password" disabled={busy} value={user} onChange={event=>setUser(event.target.value)}/></label>
    <label>Confirm opening password<input type="password" autoComplete="new-password" disabled={busy} value={userConfirm} onChange={event=>setUserConfirm(event.target.value)}/></label>
    <label>Owner password<input type="password" autoComplete="new-password" disabled={busy} value={owner} onChange={event=>setOwner(event.target.value)}/></label>
    <label>Confirm owner password<input type="password" autoComplete="new-password" disabled={busy} value={ownerConfirm} onChange={event=>setOwnerConfirm(event.target.value)}/></label>
    <p>The owner password is required and must differ from the opening password. Confirm both exactly. Passwords support up to 127 UTF-8 bytes after PDF Unicode normalization; the native check validates this before saving.</p>
    <label>Printing<select disabled={busy} value={!permissions.print?'none':permissions.printHighQuality?'high':'low'} onChange={event=>setPermissions(value=>({...value,print:event.target.value!=='none',printHighQuality:event.target.value==='high'}))}><option value="high">High quality</option><option value="low">Low resolution</option><option value="none">Not allowed</option></select></label>
    {checkbox('copyContent','Copy text and graphics')}{checkbox('editContent','Edit page content')}{checkbox('annotate','Add comments and annotations')}{checkbox('fillForms','Fill form fields')}{checkbox('assemble','Arrange pages and bookmarks')}
    <p>Allowing annotations also permits form filling. Restrictions depend on reader enforcement.</p>
   </>:<p>The copy will have no password protection. Anyone with the file can open it and use its content.</p>}
   {(denied||failure)&&<p role="alert" className="error-note">{denied||failure}</p>}
   {busy&&<p role="status" aria-live="polite">{status}</p>}{notice&&<p role="status">{notice}</p>}
  </div>
  <div className="dialog-actions"><button className="button" disabled={cancelling||finishing} onClick={()=>void cancel()}>{busy?'Cancel save':'Cancel'}</button><button className="button primary" disabled={busy||!!invalid||!!denied||!adapter.startPdfSave} onClick={()=>void start()}>Choose file and save…</button></div>
 </Modal>;
}
