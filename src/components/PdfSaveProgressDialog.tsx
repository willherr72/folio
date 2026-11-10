import {Modal} from './Modal';
import type {PdfSaveProgress,PdfSaveStage} from '../editor/pdf-save';
import './release-dialogs.css';
export interface PdfSaveView{progress:PdfSaveProgress|null;cancelling:boolean;finishing:boolean;error:string|null}
const labels:Record<PdfSaveStage,string>={choosingFile:'Choose a destination in the Save dialog. Cancel that dialog to return.',preparing:'Preparing pages',checking:'Checking document permissions and content…',copying:'Copying pages',preserving:'Preserving forms, comments, and links…',applying:'Applying page edits',renderingEdits:'Rendering text, images, and ink',preparingFile:'Preparing the PDF file…',securing:'Applying security settings…',writing:'Writing the PDF file…',finishing:'Finishing save…'};
export function PdfSaveProgressDialog({view,onCancel}:{view:PdfSaveView;onCancel():void}){
 const {progress,cancelling,finishing,error}=view;
 const counted=!!progress&&['preparing','copying','applying','renderingEdits'].includes(progress.stage)&&!cancelling&&!finishing;
 const status=finishing?'Finishing save…':cancelling?'Cancelling… the current operation may need to finish first.':progress?`${labels[progress.stage]}${counted?`: ${progress.completed} of ${progress.total}`:''}`:'Starting save…';
 const cancel=()=>{if(!cancelling&&!finishing)onCancel();};
 return <Modal className="release-dialog" title="Saving PDF" onClose={cancel}>
  <div className="release-form">
   <p role="status" aria-live="polite">{status}</p>
   <progress aria-label="PDF save progress" max={counted?progress!.total:undefined} value={counted?progress!.completed:undefined} style={{width:'100%'}}/>
   {finishing&&<p>The file is being finalized. Please wait for confirmation.</p>}
   {error&&<p role="alert" className="error-note">{error}</p>}
  </div>
  <div className="dialog-actions"><button className="button" disabled={cancelling||finishing} onClick={cancel}>Cancel save</button></div>
 </Modal>;
}
