import { useEffect, useState } from "react";
import type { Overlay, TextOverlay } from "../editor/types";
import { MIN_AREA_SIZE, resizeAreaTo } from "../editor/text-area-geometry";
import "./text-area-controls.css";

function AreaNumber({label,value,min,max,step=1,onCommit}:{label:string;value:number;min:number;max?:number;step?:number;onCommit(value:number):void}) {
  const [draft,setDraft]=useState<string|null>(null);
  useEffect(()=>setDraft(null),[value]);
  return <label className="field-label">{label}<input type="number" min={min} max={max} step={step} value={draft??Number(value.toFixed(2))}
    onChange={event=>setDraft(event.target.value)} onBlur={()=>{
      const number=Number(draft);setDraft(null);
      if(draft!==null&&draft!==""&&Number.isFinite(number)&&number!==value) onCommit(number);
    }} onKeyDown={event=>{
      if(event.key==="Enter"){event.preventDefault();event.stopPropagation();event.currentTarget.blur();}
      if(event.key==="Escape"){event.preventDefault();event.stopPropagation();setDraft(null);}
    }}/></label>;
}

export function TextAreaProperties({overlay,pageWidth,pageHeight,onChange}:{overlay:TextOverlay;pageWidth:number;pageHeight:number;onChange(update:(value:Overlay)=>Overlay):void}) {
  const area=overlay.area!;
  const minimum=Math.max(MIN_AREA_SIZE,area.inset*2+1);
  const size=(dimension:"width"|"height",raw:string)=>{
    if(!raw) return;
    const width=dimension==="width"?Number(raw):area.width,height=dimension==="height"?Number(raw):area.height;
    if(resizeAreaTo(overlay,width,height,pageWidth,pageHeight)===overlay) return;
    onChange(value=>value.type==="text"?resizeAreaTo(value,width,height,pageWidth,pageHeight):value);
  };
  const setting=(patch:Partial<NonNullable<TextOverlay["area"]>>)=>onChange(value=>value.type==="text"&&value.area?{...value,area:{...value.area,...patch}}:value);
  return <section className="property-section"><h3>Text area</h3>
    <div className="area-dimensions">
      <AreaNumber key={`${overlay.id}-width`} label="Area width (pt)" min={minimum} value={area.width} onCommit={value=>size("width",String(value))}/>
      <AreaNumber key={`${overlay.id}-height`} label="Area height (pt)" min={minimum} value={area.height} onCommit={value=>size("height",String(value))}/>
    </div>
    <label className="field-label">Alignment<select value={area.alignment} onChange={event=>setting({alignment:event.target.value as typeof area.alignment})}>
      <option value="left">Left</option><option value="center">Center</option><option value="right">Right</option>
    </select></label>
    <AreaNumber key={`${overlay.id}-spacing`} label="Line spacing" min={1} max={3} step={0.1} value={area.lineSpacing} onCommit={value=>setting({lineSpacing:Math.max(1,Math.min(3,value))})}/>
    <AreaNumber key={`${overlay.id}-inset`} label="Inset (pt)" min={0} max={Math.min(area.width,area.height)/2-0.5} value={area.inset} onCommit={value=>setting({inset:Math.max(0,Math.min(Math.min(area.width,area.height)/2-0.5,value))})}/>
    <p>Drag a corner to resize, or focus a handle and use arrow keys. Shift makes larger steps. Resizing rewraps the text and keeps its size.</p>
  </section>;
}
