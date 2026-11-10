import {useState} from "react";
import {Modal} from "./Modal";
import {selectPrintPages,type PrintOptions} from "../editor/printing";
import type {PagePlan} from "../editor/types";
import {pageAllows} from "../editor/protection";
import "./release-dialogs.css";
export function PrintDialog({pages,currentPageId,onClose,onPrint}:{pages:PagePlan[];currentPageId:string|null;onClose():void;onPrint(pages:PagePlan[],options:PrintOptions):void}){
 const [mode,setMode]=useState<"all"|"current"|"range">("all"),[range,setRange]=useState("");
 const [scale,setScale]=useState<PrintOptions["scale"]>("fit"),[orientation,setOrientation]=useState<PrintOptions["orientation"]>("portrait");
 let selected:PagePlan[]=[];let error:string|null=null;
 try{selected=selectPrintPages(pages,mode,currentPageId,range);}catch(value){error=String(value instanceof Error?value.message:value);}
 if(!error&&selected.some(page=>!pageAllows(page,"print")))error="Some selected pages do not allow printing. Choose permitted pages or unlock the document with its owner password.";
 return <Modal className="release-dialog" title="Print document" description="Print your current edits. Choose a printer and paper in the Windows dialog next." onClose={onClose}>
 <div className="release-form">
 <label>Pages<select value={mode} onChange={event=>setMode(event.target.value as typeof mode)}><option value="all">All pages ({pages.length})</option><option value="current">Current page</option><option value="range">Page range</option></select></label>
 {mode==="range"&&<label>Page range<input autoFocus value={range} onChange={event=>setRange(event.target.value)} placeholder="1, 3-5" aria-invalid={!!error}/></label>}
 <label>Sizing<select value={scale} onChange={event=>setScale(event.target.value as typeof scale)}><option value="fit">Fit to printable area</option><option value="actual">Actual size (100%)</option></select></label>
 <label>Orientation<select value={orientation} onChange={event=>setOrientation(event.target.value as typeof orientation)}><option value="portrait">Portrait</option><option value="landscape">Landscape</option></select></label>
 {scale==="actual"&&<p>Content outside the printer’s printable area may be clipped.</p>}
 {error&&<p className="error-note" role="alert">{error}</p>}
 {!error&&selected.some(page=>!pageAllows(page,"printHighQuality"))&&<p>This PDF limits printing to 150 dpi.</p>}
 <p>{selected.length} page{selected.length===1?"":"s"} selected. Printer properties can override orientation.</p>
 </div>
 <div className="dialog-actions"><button className="button" onClick={onClose}>Cancel</button><button className="button primary" disabled={!!error} onClick={()=>onPrint(selected,{scale,orientation})}>Choose printer…</button></div>
 </Modal>;
}
