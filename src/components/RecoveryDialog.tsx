import {Modal} from "./Modal";
import "./release-dialogs.css";
export function RecoveryDialog({count,error,busy,onRestore,onDiscard,onSkip}:{count:number;error:string|null;busy:boolean;onRestore():void;onDiscard():void;onSkip():void}){
 return <Modal className="release-dialog" title={error?"Recovery needs attention":"Restore your workspace?"} description={error??("Folio found "+count+" document"+(count===1?"":"s")+" from an interrupted session. Restore your edits and page positions.")} onClose={()=>{}}>
 <p className="recovery-description">{error?"You can retry, discard the saved recovery data, or continue with recovery disabled for this session.":"Source files remain untouched. Undo history starts fresh after recovery."}</p>
 <div className="dialog-actions">
 {error&&<button className="button" disabled={busy} onClick={onSkip}>Continue without recovery</button>}
 <button className="button" disabled={busy} onClick={onDiscard}>Discard recovery</button>
 <button className="button primary" disabled={busy} onClick={onRestore}>{busy?"Working…":error?"Retry":"Restore workspace"}</button>
 </div></Modal>;
}
