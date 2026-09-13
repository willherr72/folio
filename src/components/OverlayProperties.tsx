import { useCustomFont } from "../editor/use-font-resources";
import { customTextError } from "../editor/custom-fonts";
import { useLayoutEffect, useRef, useState } from "react";
import { useShapedText } from "../editor/shaped-text";
import { Trash2 } from "lucide-react";
import type { Overlay } from "../editor/types";
import { TEXT_FONTS, textFont, type TextFontName } from "../editor/text-fonts";
import { resizeInk } from "../editor/geometry";

export function OverlayProperties({ disabled = false, overlay, onChange, onDelete, autoEdit, onAutoEdited, pageWidth, pageHeight, onChooseFont }: {
  overlay: Overlay; onChange(update: (value: Overlay) => Overlay): void; onDelete(): void;
  onChooseFont?: () => void;
  disabled?: boolean;
  autoEdit: boolean; onAutoEdited(): void; pageWidth:number; pageHeight:number;
}) {
  const custom = useCustomFont(overlay.type === "text" ? overlay.fontId : undefined);
  const shaped = useShapedText(overlay.type === "text" && overlay.shaping && overlay.fontId ? overlay : undefined);
  const fontError = overlay.type === "text" && !overlay.shaping && overlay.fontId ? custom?.status === "ready" ? customTextError(custom.info!, overlay.text) : custom?.error : null;
  const savedText = overlay.type === "ink" ? "" : overlay.text ?? "";
  const [draft, setDraft] = useState(savedText);
  const composing = useRef(false);
  const lastCommitted = useRef(savedText);
  useLayoutEffect(() => {
    composing.current = false;
    lastCommitted.current = savedText;
    setDraft(savedText);
  }, [overlay.id, savedText, disabled]);
  const commitText = (text: string) => {
    if (disabled || text === lastCommitted.current) return;
    lastCommitted.current = text;
    const id = overlay.id;
    onChange(value => value.id === id && value.type !== "ink" ? { ...value, text } : value);
  };
  const contentRef = useRef<HTMLTextAreaElement>(null);
  useLayoutEffect(() => {
    if (!disabled && autoEdit && contentRef.current) { contentRef.current.focus(); contentRef.current.select(); onAutoEdited(); }
  }, [disabled, autoEdit, overlay.id, onAutoEdited]);
  const title = overlay.type === "text" ? "Text" : overlay.type === "ink" ? "Ink" : overlay.type === "highlight" ? "Highlight" : "Comment";
  const points = overlay.type === "ink" ? overlay.paths.flat() : [];
  const extent = points.reduce((box,point)=>({min:Math.min(box.min,point.x),max:Math.max(box.max,point.x)}),{min:Infinity,max:-Infinity});
  const width = points.length ? extent.max-extent.min : 0;
  return <fieldset disabled={disabled} style={{ display: "contents" }}>
    <section className="property-section"><h3>{title}</h3>
      {overlay.type !== "ink" && <label className="field-label">{overlay.type === "text" ? "Content" : overlay.type === "highlight" ? "Highlight note" : "Comment"}
        <textarea ref={contentRef} maxLength={32768} value={draft} rows={4} placeholder={overlay.type === "highlight" ? "Add an optional note…" : "Write a comment…"} onCompositionStart={() => { if (!disabled) composing.current = true; }}
          onCompositionEnd={event => {
            if (disabled || !composing.current) return;
            composing.current = false;
            const text = event.currentTarget.value;
            setDraft(text);
            commitText(text);
          }}
          onKeyDown={event => { if (composing.current || event.nativeEvent.isComposing || event.keyCode === 229) event.stopPropagation(); }}
          onChange={event => {
            if (disabled) return;
            const text = event.currentTarget.value;
            setDraft(text);
            if (!composing.current) commitText(text);
          }}/>
      </label>}
      {overlay.type === "text" && <label className="field-label">Font<select value={overlay.fontId ? "custom" : overlay.fontName ?? "Helvetica"} onChange={event => onChange(value => value.type === "text" ? { ...value, fontId: undefined, shaping: undefined, fontName: event.target.value as TextFontName } : value)}>
        {overlay.fontId && <option value="custom">{custom?.info?.name ?? "Custom font"}</option>}
        {TEXT_FONTS.map(font => <option key={font.name} value={font.name}>{font.label}</option>)}
      </select></label>}
      {overlay.type === "text" && onChooseFont && <button className="button" onClick={onChooseFont}>More fonts…</button>}
      {overlay.type === "text" && onChooseFont && <>
        <label className="field-label"><span><input type="checkbox" checked={!!overlay.shaping} onChange={event => {
          const checked = event.currentTarget.checked;
          onChange(value => value.type === "text" ? { ...value, shaping: checked ? { version: 1, direction: "auto", ligatures: true } : undefined } : value);
        }}/> Shaped text</span></label>
        {overlay.shaping && <>
          <label className="field-label">Direction<select value={overlay.shaping.direction} onChange={event => {
            const direction = event.currentTarget.value as "auto" | "ltr" | "rtl";
            onChange(value => value.type === "text" && value.shaping ? { ...value, shaping: { ...value.shaping, direction } } : value);
          }}><option value="auto">Automatic</option><option value="ltr">Left to right</option><option value="rtl">Right to left</option></select></label>
          <label className="field-label"><span><input type="checkbox" checked={overlay.shaping.ligatures} onChange={event => {
            const ligatures = event.currentTarget.checked;
            onChange(value => value.type === "text" && value.shaping ? { ...value, shaping: { ...value.shaping, ligatures } } : value);
          }}/> Ligatures</span></label>
          <p>Single-line text with the selected font's shaping.</p>
        </>}
      </>}
      {overlay.type === "text" && overlay.shaping && overlay.fontId && (!shaped || shaped.status === "loading") && <p role="status">Preparing shaped text…</p>}
      {overlay.type === "text" && overlay.shaping && !overlay.fontId && <p role="alert">Choose a custom font for shaped text.</p>}
      {shaped?.status === "error" && <><p role="alert">{shaped.error}</p>{shaped.retry && <button className="button" onClick={shaped.retry}>Retry preview</button>}</>}
      {fontError && <p role="alert">{fontError}</p>}
      {overlay.type === "text" && <label className="field-label">Size<input type="number" min="6" max="96" value={overlay.fontSize} onChange={event=>onChange(value=>value.type==="text" ? {...value,fontSize:Math.max(6,Math.min(96,Number(event.target.value)))} : value)}/></label>}
      <label className="field-label">{overlay.type === "ink" ? "Ink color" : "Color"}<span className="color-input"><input type="color" value={overlay.color} onChange={event=>onChange(value=>({...value,color:event.target.value.toUpperCase()}))}/><code>{overlay.color}</code></span></label>
      {overlay.type === "ink" && <>
        <label className="field-label">Stroke width · {overlay.strokeWidth} pt<input type="range" min="0.5" max="20" step="0.5" value={overlay.strokeWidth} onChange={event=>onChange(value=>value.type==="ink" ? {...value,strokeWidth:Number(event.target.value)} : value)}/></label>
        <label className="field-label">Width (pt)<input aria-label="Ink width" type="number" min="1" max={pageWidth} disabled={width<=0} value={Math.round(width*10)/10} onChange={event=>onChange(value=>value.type==="ink" ? resizeInk(value,Number(event.target.value),pageWidth,pageHeight) : value)}/></label>
        <p>Width keeps the proportions and fits the page.</p>
      </>}
      {overlay.type === "text" && <p>{overlay.fontId ? custom?.info?.name ?? "Custom font" : textFont(overlay.fontName).label} · {overlay.text.split("\n").length} {overlay.text.includes("\n") ? "lines" : "line"}</p>}
    </section>
    <section className="property-section"><h3>{overlay.type === "highlight" ? "Text anchor" : "Position"}</h3>
      <p>{overlay.type === "highlight" ? "This highlight stays anchored to its text. Select it from the review list to edit its note or color." : "Drag this item directly on the page to move it."}</p>
      <button className="danger-action" onClick={onDelete}><Trash2 size={16}/>Delete {overlay.type}</button>
    </section>
  </fieldset>;
}