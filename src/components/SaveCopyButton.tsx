import {useCallback,useEffect,useId,useLayoutEffect,useRef,useState} from "react";
import {createPortal} from "react-dom";
import {ChevronDown,Download} from "lucide-react";
import "./save-options.css";
export function SaveCopyButton({disabled,demo,onSave,onExtract,onSplit,onExportImages,onExportText,onSecurity,onProperties,allowTextExport=true,allowImageExport=true,allowFlatten=true,allowExtract=true}:{disabled:boolean;demo:boolean;onSave(flatten:boolean):void;onExtract?():void;onSplit?():void;onExportImages?():void;onExportText?():void;onSecurity?():void;onProperties?():void;allowTextExport?:boolean;allowImageExport?:boolean;allowFlatten?:boolean;allowExtract?:boolean}){
 const [open,setOpen]=useState(false);
 const [position,setPosition]=useState({top:0,left:0});
 const trigger=useRef<HTMLButtonElement>(null),menu=useRef<HTMLDivElement>(null);
 const id=useId();
 const updatePosition=useCallback(()=>{
  const rect=trigger.current?.getBoundingClientRect();
  if(!rect)return;
  const next={top:rect.bottom+8,left:Math.max(8,Math.min(rect.right-320,window.innerWidth-328))};
  setPosition(current=>current.top===next.top&&current.left===next.left?current:next);
 },[]);
 useLayoutEffect(()=>{
  if(!open)return;
  updatePosition();
  menu.current?.querySelector<HTMLButtonElement>('button')?.focus({preventScroll:true});
 },[open,updatePosition]);
 useEffect(()=>{if(disabled)setOpen(false);},[disabled]);
 useEffect(()=>{
  if(!open)return;
  const outside=(event:PointerEvent)=>{if(!menu.current?.contains(event.target as Node)&&!trigger.current?.contains(event.target as Node))setOpen(false);};
  const close=()=>setOpen(false);
  document.addEventListener('pointerdown',outside);window.addEventListener('resize',close);window.addEventListener('scroll',updatePosition,true);
  return()=>{document.removeEventListener('pointerdown',outside);window.removeEventListener('resize',close);window.removeEventListener('scroll',updatePosition,true);};
 },[open,updatePosition]);
 const choose=(flatten:boolean)=>{setOpen(false);onSave(flatten);};
 return <div className={demo?'save-copy-control demo':'save-copy-control'}>
  <button className="button primary export" title={demo?undefined:"Save an editable PDF copy (Ctrl+S)"} disabled={disabled} onClick={()=>choose(false)}><Download size={17}/>{demo?"Export demo plan":"Save a copy"}</button>
  {!demo&&<button ref={trigger} className="button primary save-options-trigger" aria-label="Save options" title="Save options" aria-haspopup="menu" aria-expanded={open} aria-controls={open?id:undefined} disabled={disabled} onClick={()=>setOpen(value=>!value)}><ChevronDown size={16}/></button>}
  {open&&createPortal(<div id={id} ref={menu} className="save-options-menu" role="menu" aria-label="Save options" style={position} onKeyDown={event=>{
    const buttons=Array.from(menu.current?.querySelectorAll<HTMLButtonElement>('button:not([disabled])')??[]);
    if(event.key==='Escape'){event.preventDefault();event.stopPropagation();setOpen(false);trigger.current?.focus();}
    else if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){
      event.preventDefault();const current=buttons.indexOf(document.activeElement as HTMLButtonElement);
      buttons[event.key==='Home'?0:event.key==='End'?buttons.length-1:(current+(event.key==='ArrowDown'?1:-1)+buttons.length)%buttons.length]?.focus();
    }else if(event.key==='Tab'){
      // Resume native tab order beside the trigger, not at the portal's body-end position.
      trigger.current?.focus({preventScroll:true});setOpen(false);
    }
  }}>
   <button role="menuitem" aria-label="Save editable PDF" onClick={()=>choose(false)}><strong>Save editable PDF</strong><span>Keep text, images, drawings, and signatures editable in Folio. No original files or sidecars needed.</span></button>
   <button role="menuitem" aria-label="Flatten text, images, and ink" disabled={!allowFlatten} title={allowFlatten?undefined:"This PDF does not permit changing page content."} onClick={()=>choose(true)}><strong>Flatten text, images, and ink</strong><span>{allowFlatten?"Keep their appearance as page content without editing handles. Highlights and comments remain annotations.":"The PDF’s permissions do not allow flattening these annotations into page content."}</span></button>
   {onProperties&&<button role="menuitem" aria-label="Document properties…" onClick={()=>{setOpen(false);onProperties();}}><strong>Document properties…</strong><span>Inspect and edit document information, or remove document properties.</span></button>}
   {onSecurity&&<button role="menuitem" aria-label="Security settings…" onClick={()=>{setOpen(false);onSecurity();}}><strong>Security settings…</strong><span>Save a copy with new passwords and permissions, or remove password protection.</span></button>}
   {onExportText&&<button role="menuitem" aria-label="Export text…" disabled={!allowTextExport} title={allowTextExport?undefined:"This PDF does not permit copying content."} onClick={()=>{setOpen(false);onExportText();}}><strong>Export text…</strong><span>Save selected edited pages as UTF-8 text, with optional form values.</span></button>}
   {onSplit&&<button role="menuitem" aria-label="Split PDF…" disabled={!allowExtract} title={allowExtract?undefined:"This PDF does not permit splitting pages."} onClick={()=>{setOpen(false);onSplit();}}><strong>Split PDF…</strong><span>Create editable PDFs by page count or bookmarks</span></button>}
   {onExportImages&&<button role="menuitem" aria-label="Export images…" disabled={!allowImageExport} title={allowImageExport?undefined:"This PDF does not permit copying page content."} onClick={()=>{setOpen(false);onExportImages();}}><strong>Export images…</strong><span>Save selected edited pages as PNG or JPEG at your chosen resolution.</span></button>}
   {onExtract&&<button role="menuitem" aria-label="Extract pages…" disabled={!allowExtract} title={allowExtract?undefined:"This PDF does not permit extracting pages."} onClick={()=>{setOpen(false);onExtract();}}><strong>Extract pages…</strong><span>{allowExtract?"Save current pages or a range as a separate editable PDF.":"The PDF’s permissions do not allow extracting pages."}</span></button>}
  </div>,document.body)}
 </div>;
}
