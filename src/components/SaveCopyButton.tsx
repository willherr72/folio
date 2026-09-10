import {useCallback,useEffect,useId,useLayoutEffect,useRef,useState} from "react";
import {createPortal} from "react-dom";
import {ChevronDown,Download} from "lucide-react";
import "./save-options.css";
export function SaveCopyButton({disabled,demo,onSave}:{disabled:boolean;demo:boolean;onSave(flatten:boolean):void}){
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
    const buttons=Array.from(menu.current?.querySelectorAll<HTMLButtonElement>('button')??[]);
    if(event.key==='Escape'){event.preventDefault();event.stopPropagation();setOpen(false);trigger.current?.focus();}
    else if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){
      event.preventDefault();const current=buttons.indexOf(document.activeElement as HTMLButtonElement);
      buttons[event.key==='Home'?0:event.key==='End'?buttons.length-1:(current+(event.key==='ArrowDown'?1:-1)+buttons.length)%buttons.length]?.focus();
    }else if(event.key==='Tab'){
      // Resume native tab order beside the trigger, not at the portal's body-end position.
      trigger.current?.focus({preventScroll:true});setOpen(false);
    }
  }}>
   <button role="menuitem" aria-label="Save editable PDF" onClick={()=>choose(false)}><strong>Save editable PDF</strong><span>Keep text, drawings, and signatures editable in Folio. No original files or sidecars needed.</span></button>
   <button role="menuitem" aria-label="Flatten text and ink" onClick={()=>choose(true)}><strong>Flatten text and ink</strong><span>Keep their appearance as page content without editing handles. Highlights and comments remain annotations.</span></button>
  </div>,document.body)}
 </div>;
}