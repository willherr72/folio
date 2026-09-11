import { useLayoutEffect, useRef } from "react";
import { Trash2 } from "lucide-react";
import type { Overlay } from "../editor/types";
import { TEXT_FONTS, textFont, type TextFontName } from "../editor/text-fonts";
import { resizeInk } from "../editor/geometry";

export function OverlayProperties({ overlay, onChange, onDelete, autoEdit, onAutoEdited, pageWidth, pageHeight }: {
  overlay: Overlay; onChange(update: (value: Overlay) => Overlay): void; onDelete(): void;
  autoEdit: boolean; onAutoEdited(): void; pageWidth:number; pageHeight:number;
}) {
  const contentRef = useRef<HTMLTextAreaElement>(null);
  useLayoutEffect(() => {
    if (autoEdit && contentRef.current) { contentRef.current.focus(); contentRef.current.select(); onAutoEdited(); }
  }, [autoEdit, overlay.id, onAutoEdited]);
  const title = overlay.type === "text" ? "Text" : overlay.type === "ink" ? "Ink" : overlay.type === "highlight" ? "Highlight" : "Comment";
  const points = overlay.type === "ink" ? overlay.paths.flat() : [];
  const extent = points.reduce((box,point)=>({min:Math.min(box.min,point.x),max:Math.max(box.max,point.x)}),{min:Infinity,max:-Infinity});
  const width = points.length ? extent.max-extent.min : 0;
  return <>
    <section className="property-section"><h3>{title}</h3>
      {overlay.type !== "ink" && <label className="field-label">{overlay.type === "text" ? "Content" : overlay.type === "highlight" ? "Highlight note" : "Comment"}
        <textarea ref={contentRef} maxLength={32768} value={overlay.text??""} rows={4} placeholder={overlay.type === "highlight" ? "Add an optional note…" : "Write a comment…"} onChange={event=>onChange(value=>value.type!=="ink" ? {...value,text:event.target.value} : value)}/>
      </label>}
      {overlay.type === "text" && <label className="field-label">Font<select value={overlay.fontName ?? "Helvetica"} onChange={event => onChange(value => value.type === "text" ? { ...value, fontName: event.target.value as TextFontName } : value)}>
        {TEXT_FONTS.map(font => <option key={font.name} value={font.name}>{font.label}</option>)}
      </select></label>}
      {overlay.type === "text" && <label className="field-label">Size<input type="number" min="6" max="96" value={overlay.fontSize} onChange={event=>onChange(value=>value.type==="text" ? {...value,fontSize:Math.max(6,Math.min(96,Number(event.target.value)))} : value)}/></label>}
      <label className="field-label">{overlay.type === "ink" ? "Ink color" : "Color"}<span className="color-input"><input type="color" value={overlay.color} onChange={event=>onChange(value=>({...value,color:event.target.value.toUpperCase()}))}/><code>{overlay.color}</code></span></label>
      {overlay.type === "ink" && <>
        <label className="field-label">Stroke width · {overlay.strokeWidth} pt<input type="range" min="0.5" max="20" step="0.5" value={overlay.strokeWidth} onChange={event=>onChange(value=>value.type==="ink" ? {...value,strokeWidth:Number(event.target.value)} : value)}/></label>
        <label className="field-label">Width (pt)<input aria-label="Ink width" type="number" min="1" max={pageWidth} disabled={width<=0} value={Math.round(width*10)/10} onChange={event=>onChange(value=>value.type==="ink" ? resizeInk(value,Number(event.target.value),pageWidth,pageHeight) : value)}/></label>
        <p>Width keeps the proportions and fits the page.</p>
      </>}
      {overlay.type === "text" && <p>{textFont(overlay.fontName).label} · {overlay.text.split("\n").length} {overlay.text.includes("\n") ? "lines" : "line"}</p>}
    </section>
    <section className="property-section"><h3>{overlay.type === "highlight" ? "Text anchor" : "Position"}</h3>
      <p>{overlay.type === "highlight" ? "This highlight stays anchored to its text. Select it from the review list to edit its note or color." : "Drag this item directly on the page to move it."}</p>
      <button className="danger-action" onClick={onDelete}><Trash2 size={16}/>Delete {overlay.type}</button>
    </section>
  </>;
}