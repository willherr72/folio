import {useEffect,useState} from "react";
import type {FolioAdapter} from "../editor/adapter";
import type {AnnotationRect,EditableTextRun,Overlay} from "../editor/types";
import {MAX_PARAGRAPH_PIECES,NO_PARAGRAPH_OVERLAYS,renderParagraphPage} from "../editor/paragraph-preview";
import {ParagraphAreaMove} from "./ParagraphAreaMove";
import "./paragraph-page-preview.css";

interface Props {
  adapter:FolioAdapter;
  sourceId:string;
  pageIndex:number;
  pageWidth:number;
  pageHeight:number;
  overlays?:Overlay[];
  crop?:AnnotationRect;
  runs:EditableTextRun[];
  selected:number[];
  selectable:boolean;
  /** Temporarily show the original while retaining the comparison preference. */
  forceOriginal?:boolean;
  onToggle(index:number):void;
  bounds?:AnnotationRect;
  onBoundsChange?(bounds:AnnotationRect):void;
  onDraggingChange?(dragging:boolean):void;
  /** Borrowed URL; the dialog owns both this URL and its source. */
  candidateUrl?:string;
}
const validRect=(rect:AnnotationRect)=>Object.values(rect).every(Number.isFinite) && rect.width>0 && rect.height>0;
const revoke=(url:string|null)=>{if(url?.startsWith("blob:"))URL.revokeObjectURL(url);};

export function ParagraphPagePreview({adapter,sourceId,pageIndex,pageWidth,pageHeight,crop,overlays=NO_PARAGRAPH_OVERLAYS,runs,selected,selectable,forceOriginal=false,onToggle,bounds,candidateUrl,onBoundsChange,onDraggingChange}:Props) {
  const [original,setOriginal]=useState<string|null>(null);
  const [error,setError]=useState<string|null>(null);
  const [zoom,setZoom]=useState(100);
  const [comparison,setComparison]=useState<"original"|"edited">("edited");
  const [dragging,setDragging]=useState(false);
  useEffect(()=>{
    let active=true;let owned:string|null=null;
    setOriginal(null);setError(null);
    void renderParagraphPage(adapter,sourceId,pageIndex,pageWidth,pageHeight,overlays).then(url=>{
      if(!active){revoke(url);return;}
      owned=url;setOriginal(url);
    }).catch(error=>{if(active)setError(error instanceof Error?error.message:String(error));});
    return ()=>{active=false;revoke(owned);};
  },[adapter,sourceId,pageIndex,pageWidth,pageHeight,overlays]);
  useEffect(()=>{setComparison("edited");},[candidateUrl]);
  const edited=!!candidateUrl && !forceOriginal && comparison==="edited";
  const url=edited?candidateUrl:original;
  const canSelect=selectable && !edited;
  return <section className="paragraph-page-preview" aria-label="Page preview">
    <div className="paragraph-preview-toolbar">
      <div className="paragraph-comparison" aria-label="Compare page">
        <button type="button" className="button" aria-pressed={!edited} disabled={!original||dragging||forceOriginal} onClick={()=>setComparison("original")}>Original</button>
        <button type="button" className="button" aria-pressed={edited} disabled={!candidateUrl||dragging||forceOriginal} onClick={()=>setComparison("edited")}>Edited</button>
      </div>
      <label>Zoom<select aria-label="Preview zoom" disabled={dragging} value={zoom} onChange={event=>setZoom(Number(event.target.value))}>
        <option value={100}>Fit width</option><option value={150}>150%</option><option value={200}>200%</option><option value={300}>300%</option>
      </select></label>
    </div>
    <p className="paragraph-preview-caption">{edited?"Edited PDF · review before applying":canSelect?"Original PDF · click text pieces to select them":"Original PDF · selected paragraph area"}</p>
    {crop&&<p className="paragraph-preview-caption">Full source shown for editing. The dashed outline marks the saved and printed crop.</p>}
    {bounds&&onBoundsChange&&!edited&&<p className="paragraph-preview-caption">Drag the outlined area to move it, or enter Left and Top. Escape cancels a drag.</p>}
    {error && <p role="alert" className="existing-text-error">Original page preview unavailable: {error}. You can still select pieces from the list.</p>}
    {!url && !error && <p role="status">Rendering original page…</p>}
    {url && <div className="paragraph-page-scroll" tabIndex={0} aria-label="Scrollable PDF preview">
      <div className="paragraph-page-surface" data-testid="paragraph-page-surface" style={{width:`${zoom}%`,aspectRatio:`${pageWidth} / ${pageHeight}`}}>
        <img src={url} alt={edited?"Paragraph PDF preview":"Original PDF page"} draggable={false}/>
        <svg className="paragraph-page-targets" viewBox={`0 0 ${pageWidth} ${pageHeight}`} aria-label="Paragraph selection on page">
          {crop&&<rect {...crop} fill="none" stroke="#2874b8" strokeWidth={2} strokeDasharray="6 4" vectorEffect="non-scaling-stroke" pointerEvents="none" aria-label="Retained crop area"/>}
          {canSelect && runs.filter(run=>validRect(run.bounds)).map(run=>{
            const pressed=selected.includes(run.objectIndex),disabled=!pressed&&selected.length>=MAX_PARAGRAPH_PIECES;
            return <rect key={run.objectIndex}
              {...run.bounds} role="button" tabIndex={disabled?-1:0} aria-label={`Select text piece: ${run.text}`} aria-pressed={pressed} aria-disabled={disabled}
              className={`paragraph-piece-target${pressed?" selected":""}`}
              onClick={()=>{if(!disabled)onToggle(run.objectIndex);}} onKeyDown={event=>{if(event.key==="Enter" || event.key===" "){event.preventDefault();if(!disabled)onToggle(run.objectIndex);}}}/>;
          })}
          {bounds && validRect(bounds) && (onBoundsChange&&!edited
            ?<ParagraphAreaMove bounds={bounds} pageWidth={pageWidth} pageHeight={pageHeight} onChange={onBoundsChange} onDraggingChange={value=>{setDragging(value);onDraggingChange?.(value);}}/>
            :<rect {...bounds} className="paragraph-area-target" aria-label="Paragraph area"/>)}
        </svg>
      </div>
    </div>}
  </section>;
}
