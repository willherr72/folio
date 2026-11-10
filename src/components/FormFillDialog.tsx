import {Modal} from "./Modal";
import {useRef} from "react";
import {FormFillEditor,type FormFillEditorProps} from "./FormFillEditor";

export function FormFillDialog(props:FormFillEditorProps){
 const busy=useRef(false);const close=()=>{if(!busy.current)props.onClose();};
 return <Modal className="form-fill-dialog" title="Fill forms" description={`Interactive fields on page ${props.pageNumber}. Calculations and validation scripts are not run.`} onClose={close}>
  <FormFillEditor {...props} preview onClose={close} onBusyChange={value=>{busy.current=value;props.onBusyChange?.(value);}}/>
 </Modal>;
}
