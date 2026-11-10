import {PreciseNumberInput} from "./PreciseNumberInput";
import {ParagraphWorkspace,useParagraphCanvasPreview} from "./ParagraphWorkspace";
import {paragraphRasterWidth,type ParagraphCanvasPresentation} from "../editor/paragraph-canvas";
import {paragraphReadAdapter,previewTextParagraph,type ParagraphOccurrence,type ParagraphPreview} from "../editor/paragraph-transaction";
import type {AnnotationRect} from "../editor/types";
import {useEffect,useMemo, useLayoutEffect, useRef, useState} from "react";
import type {FolioAdapter} from "../editor/adapter";
import type {DocumentInfo,PageContentReplacement, EditableTextRun, TextParagraphSelection,Overlay,ParagraphAlignment} from "../editor/types";
import {MAX_PARAGRAPH_PIECES,NO_PARAGRAPH_OVERLAYS,renderParagraphPage,validParagraphBounds} from "../editor/paragraph-preview";
import {ParagraphPagePreview} from "./ParagraphPagePreview";
import {Modal} from "./Modal";
import {FontPicker} from "./FontPicker";
import {customTextError,holdCustomFont,type FontInfo} from "../editor/custom-fonts";
import "./existing-text.css";

interface Props {
 canvas?:ParagraphCanvasPresentation;
 occurrence?:ParagraphOccurrence;
  adapter:FolioAdapter;
  sourceId:string;
  pageIndex:number;
  pageWidth:number;
  pageHeight:number;
  overlays?:Overlay[];crop?:AnnotationRect;
  anchor:EditableTextRun;
  /** Synchronous ownership transfer: true means the workspace owns this source. */
  onApply(document:DocumentInfo,replacement?:PageContentReplacement):boolean;
  onCancel():void;
}
type Candidate=ParagraphPreview&{url:string};
const message=(error:unknown)=>error instanceof Error?error.message:String(error);
const spacingDraft=(value:number)=>String(value);

export function TextParagraphDialog({adapter:baseAdapter,occurrence,canvas,sourceId,pageIndex,pageWidth,pageHeight,crop,overlays=NO_PARAGRAPH_OVERLAYS,anchor,onApply,onCancel}:Props) {
 const flight=useRef(false),[nativeBusy,setNativeBusy]=useState(false);
 const adapter=useMemo(()=>paragraphReadAdapter(baseAdapter,occurrence,"uniform"),[baseAdapter,occurrence]);
  const [runs,setRuns]=useState<EditableTextRun[]>([]);
  const [loading,setLoading]=useState(true);
  const [selected,setSelected]=useState([anchor.objectIndex]);
  const [selection,setSelection]=useState<TextParagraphSelection|null>(null);
  const [adjusting,setAdjusting]=useState(false);
  const [text,setText]=useState("");
  const [fontSize,setFontSize]=useState("");
  const [alignment,setAlignment]=useState<ParagraphAlignment>("left");
  const [lineSpacing,setLineSpacing]=useState("1.2");
  const [spacingEdited,setSpacingEdited]=useState(false);
  const [font,setFont]=useState<FontInfo|null>(null);
  const [color,setColor]=useState<string|null>(null);
  const [pickerOpen,setPickerOpen]=useState(false);
  const fontLease=useRef<(()=>Promise<void>)|null>(null);
  const [left,setLeft]=useState("");
  const [top,setTop]=useState("");
  const [width,setWidth]=useState("");
  const [height,setHeight]=useState("");
  const [operationWorking,setWorking]=useState(false);
  const [dragging,setDragging]=useState(false);
  const [error,setError]=useState<string|null>(null);
  const [suggestionNotice,setSuggestionNotice]=useState<string|null>(null);
  const [preview,setPreview]=useState<Candidate|null>(null);
  const candidate=useRef<Candidate|null>(null);
  const generation=useRef(0);
  const alive=useRef(false);
  const pending=useRef(false);
  const pieceGroup=useRef<HTMLFieldSetElement>(null);
  const textInput=useRef<HTMLTextAreaElement>(null);
  const focusNext=useRef<"pieces"|"text"|null>(null);
 const working=operationWorking||nativeBusy;
  const choosing=!selection||adjusting;
  const clearFont=()=>{
    const previous=fontLease.current;fontLease.current=null;
    setFont(null);setPickerOpen(false);void previous?.();
  };
  const release=(value:Candidate|null)=>{
    if (!value) return;
    if (value.url.startsWith("blob:")) URL.revokeObjectURL(value.url);
    void value.release();
  };
  const setParagraph=(result:TextParagraphSelection)=>{
    clearFont();
    setColor(null);
    setAlignment(result.alignment??"left");
    setLineSpacing(spacingDraft(result.lineSpacing??1.2));setSpacingEdited(false);
    setSelection(result);setText(result.text);setFontSize(String(result.fontSize));setLeft(String(result.bounds.x));setTop(String(result.bounds.y));setWidth(String(result.bounds.width));setHeight(String(result.bounds.height));
  };
  useEffect(()=>{
    alive.current=true;
    let active=true;
    const token=generation.current;
    clearFont();
    pending.current=false;focusNext.current=null;setAdjusting(false);setWorking(false);setLoading(true);setRuns([]);setSelected([anchor.objectIndex]);setSelection(null);setPreview(null);setSuggestionNotice(null);setError(null);
    const loadRuns=async()=>adapter.listTextRuns!(sourceId,pageIndex);
    const loadSuggestion=async()=>adapter.suggestTextParagraph?.(sourceId,pageIndex,anchor.objectIndex,anchor.text)??null;
    void Promise.allSettled([loadRuns(),loadSuggestion()]).then(([listed,suggested])=>{
      if (!active || token!==generation.current) return;
      if(listed.status==="rejected") {setError(message(listed.reason));setLoading(false);return;}
      setRuns(listed.value.runs.filter(run=>run.text.trim()));setError(listed.value.reason??null);
      if(suggested.status==="fulfilled" && suggested.value) {
        setSelected(suggested.value.objectIndices);setParagraph(suggested.value);
        setSuggestionNotice("Suggested paragraph. Adjust the selection if needed.");
      } else if(suggested.status==="rejected") {
        setSuggestionNotice("Select the paragraph pieces manually, then choose Check selection.");
        setError(message(suggested.reason));
      }
      setLoading(false);
    });
    return ()=>{
      active=false;alive.current=false;generation.current++;
      const lease=fontLease.current;fontLease.current=null;void lease?.();
      const owned=candidate.current;candidate.current=null;release(owned);
    };
  },[adapter,sourceId,pageIndex,anchor.objectIndex,anchor.text]);
  useLayoutEffect(()=>{
    const target=focusNext.current==="pieces"?pieceGroup.current:focusNext.current==="text"?textInput.current:null;
    if(target){target.focus();focusNext.current=null;}
  },[choosing,selection,loading]);
  const stopPending=()=>{
    generation.current++;pending.current=false;setWorking(false);setError(null);
  };
  const invalidate=()=>{
    stopPending();
    const owned=candidate.current;candidate.current=null;setPreview(null);release(owned);
  };
  const beginAdjustment=()=>{
    if(!selection||pending.current||dragging)return;
    stopPending();setSelected([...selection.objectIndices]);focusNext.current="pieces";setAdjusting(true);
  };
  const cancelAdjustment=()=>{
    if(!selection)return;
    stopPending();setSelected([...selection.objectIndices]);focusNext.current="text";setAdjusting(false);
  };
  const toggle=(index:number)=>{
    if(dragging||pending.current||!choosing)return;
    if(selected.length>=MAX_PARAGRAPH_PIECES && !selected.includes(index))return;
    stopPending();
    setSelected(value=>value.includes(index)?value.filter(i=>i!==index):[...value,index].sort((a,b)=>a-b));
  };
  const check=async()=>{
    if(pending.current || dragging || !selected.length || !choosing)return;
    if(selection&&selected.length===selection.objectIndices.length&&selected.every((value,index)=>value===selection.objectIndices[index])){cancelAdjustment();return;}
    stopPending();const token=generation.current;pending.current=true;setWorking(true);
    try {
      const result=await adapter.inspectTextParagraph!(sourceId,pageIndex,[...selected]);
      if(!alive.current || token!==generation.current)return;
      invalidate();setParagraph(result);setSelected(result.objectIndices);setSuggestionNotice(null);focusNext.current="text";setAdjusting(false);
    } catch(error) {if(alive.current && token===generation.current)setError(message(error));}
    finally {if(alive.current && token===generation.current){pending.current=false;setWorking(false);}}
  };
  const area={x:Number(left),y:Number(top),width:Number(width),height:Number(height)};
  const dimensionsValid=[left,top,width,height].every(value=>value.trim()!=="")&&validParagraphBounds(area,pageWidth,pageHeight);
  const chosenSize=Number(fontSize);
  const sizeValid=fontSize.trim()!==""&&Number.isFinite(chosenSize)&&chosenSize>=1&&chosenSize<=300;
  const chosenSpacing=Number(lineSpacing);
  const spacingValid=lineSpacing.trim()!==""&&Number.isFinite(chosenSpacing)&&chosenSpacing>=0.6&&chosenSpacing<=3;
  const fontError=font?customTextError(font,text):null;
  const canPreview=!choosing && !!selection && dimensionsValid && sizeValid && spacingValid && !fontError && !!text.trim() && new TextEncoder().encode(text).length<=4096;
  const makePreview=async():Promise<void>=>{
    if((canvas&&(flight.current||pickerOpen))||pending.current || dragging || !canPreview || !selection)return;
    canvasPreview.consume();
    invalidate();const token=generation.current;pending.current=true;setWorking(true);if(canvas){flight.current=true;setNativeBusy(true);}
    let owned:ParagraphPreview|null=null;let url:string|null=null;
    // The request can outlive this dialog or font choice. Its own lease keeps
    // native font registration alive until the preview has settled.
    const releaseFont=font?holdCustomFont(font.id):null;
    try {
      const args=[sourceId,pageIndex,selection.objectIndices,selection.text,text,area] as const;
      const size=chosenSize===selection.fontSize?undefined:chosenSize;
      const changedAlignment=alignment!==(selection.alignment??"left")?alignment:undefined;
      const request=color!==null?previewTextParagraph(baseAdapter,occurrence,...args,size,changedAlignment,spacingEdited?chosenSpacing:undefined,font?.id,color)
        :font?previewTextParagraph(baseAdapter,occurrence,...args,size,changedAlignment,spacingEdited?chosenSpacing:undefined,font.id):spacingEdited
        ?previewTextParagraph(baseAdapter,occurrence,...args,size,changedAlignment,chosenSpacing)
        :changedAlignment!==undefined?previewTextParagraph(baseAdapter,occurrence,...args,size,changedAlignment)
        :size===undefined?previewTextParagraph(baseAdapter,occurrence,...args):previewTextParagraph(baseAdapter,occurrence,...args,size);
      owned=await request;
      if(!alive.current || token!==generation.current)return;
      url=canvas?"":await renderParagraphPage(adapter,owned.document.id,owned.pageIndex,pageWidth,pageHeight,overlays);
      if(!alive.current || token!==generation.current)return;
      const value={...owned,url};candidate.current=value;setPreview(value);owned=null;url=null;
    } catch(error) {if(alive.current && token===generation.current)setError(message(error));}
    finally {
      if(canvas){flight.current=false;if(alive.current)setNativeBusy(false);}
      void releaseFont?.();
      if(url?.startsWith("blob:"))URL.revokeObjectURL(url);
      if(owned)void owned.release();
      if(alive.current && token===generation.current){pending.current=false;setWorking(false);}
    }
  };
 const canvasPreview=useParagraphCanvasPreview(canvas,preview,canPreview,working||dragging||pickerOpen,makePreview);
 const rasterReady=canvasPreview.ready(paragraphRasterWidth(pageWidth,canvas?.zoom??100));
  const apply=()=>{
    const value=candidate.current;if(!value || pending.current || dragging || choosing || !canPreview || !rasterReady)return;
    candidate.current=null;
    try {
      if(value.replacement?onApply(value.document,value.replacement):onApply(value.document)){
        if(value.url.startsWith("blob:"))URL.revokeObjectURL(value.url);
        setPreview(null);return;
      }
      if(alive.current){candidate.current=value;setError("The page changed. Close this dialog and select the paragraph again.");}else release(value);
    } catch(error) {if(alive.current){candidate.current=value;setError(message(error));}else release(value);}
  };
  const cancel=()=>{invalidate();onCancel();};
  const pickerView=pickerOpen?<FontPicker text={text} fontSize={chosenSize} currentFontId={font?.id} title="Choose a substitute font" description="Choose a font for this paragraph. Preview the edited PDF before applying the change." onClose={()=>{setPickerOpen(false);requestAnimationFrame(()=>textInput.current?.focus());}} onChoose={info=>{
    const previous=fontLease.current;fontLease.current=holdCustomFont(info.id);
    setFont(info);setPickerOpen(false);invalidate();void previous?.();requestAnimationFrame(()=>textInput.current?.focus());
  }}/>:null;
  const content=<>
    <div className="paragraph-body">
      <div className="existing-text-fields">
        {choosing&&<p>Select every piece of one paragraph with the same font, size, and color.</p>}
        {suggestionNotice&&(!selection||!choosing)&&<p role="status">{suggestionNotice}</p>}
        {loading?<p role="status">{adapter.suggestTextParagraph?"Finding paragraph…":"Loading page text…"}</p>:choosing&&<>
        {selection&&<p>Checking a different selection replaces this draft with the PDF’s original text and area. Cancel adjustment to keep your draft and preview.</p>}
        <fieldset ref={pieceGroup} tabIndex={-1} className="text-group-pieces">
          <legend>Paragraph pieces · {selected.length} of {MAX_PARAGRAPH_PIECES} selected</legend>
          {runs.map(run=><label key={run.objectIndex}><input type="checkbox" checked={selected.includes(run.objectIndex)} disabled={working||dragging||(selected.length>=MAX_PARAGRAPH_PIECES && !selected.includes(run.objectIndex))} onChange={()=>toggle(run.objectIndex)}/><span>{run.text}</span></label>)}
        </fieldset>
        {selected.length>=MAX_PARAGRAPH_PIECES&&<p role="status">{MAX_PARAGRAPH_PIECES} pieces selected. Deselect a piece before selecting another.</p>}
        <div className="paragraph-selection-actions">
          <button className="button" disabled={working || dragging || !selected.length} onClick={()=>void check()}>Check selection</button>
          {selection&&<button className="button" onClick={cancelAdjustment}>Cancel adjustment</button>}
        </div>
        </>}
        {selection && !choosing && <>
          <div className="paragraph-editor-heading">
            <p className="existing-text-font">{selection.fontDisplayName??selection.fontName} · {Number(selection.fontSize.toFixed(2))} pt · {selection.objectIndices.length} {selection.objectIndices.length===1?"piece":"pieces"}</p>
            <button className="button" disabled={dragging||working} onClick={beginAdjustment}>Adjust selection</button>
          </div>
          <label>Paragraph text<textarea ref={textInput} rows={6} disabled={dragging} value={text} maxLength={4096} onChange={event=>{invalidate();setText(event.target.value);}}/></label>
          <div className="existing-text-font-choice">
            <div><span>Replacement font</span><strong>{font?.name??"Original PDF font"}</strong></div>
            <button className="button" disabled={dragging||working||!sizeValid} onClick={()=>setPickerOpen(true)}>Choose substitute font…</button>
            {font&&<button className="existing-text-original-button" disabled={dragging} onClick={()=>{invalidate();clearFont();}}>Use original font</button>}
          </div>
          {fontError&&<p className="existing-text-error" role="alert">{fontError}</p>}
          <div className="paragraph-dimensions">
            <label>Font size (pt)<PreciseNumberInput disabled={dragging} min="1" max="300" step="any" value={fontSize} onChange={event=>{invalidate();setFontSize(event.target.value);}}/></label>
            <button className="button" disabled={dragging||chosenSize===selection.fontSize} onClick={()=>{invalidate();setFontSize(String(selection.fontSize));}}>Reset font size</button>
          </div>
          {!sizeValid&&<p className="existing-text-error">Use a font size from 1 to 300 pt.</p>}
          <div className="paragraph-dimensions">
            <label>Text color<input type="color" disabled={dragging} value={color??selection.color??"#000000"} onChange={event=>{invalidate();setColor(event.target.value.toUpperCase());}}/></label>
            <button className="button" disabled={dragging||color===null} onClick={()=>{invalidate();setColor(null);}}>Use original color</button>
          </div>
          <p>{color??selection.color??"Original PDF color"}</p>
          <label>Paragraph alignment<select disabled={dragging} value={alignment} onChange={event=>{invalidate();setAlignment(event.target.value as ParagraphAlignment);}}><option value="left">Left</option><option value="center">Center</option><option value="right">Right</option><option value="justify">Justify</option></select></label>
          {alignment==="justify"&&<p>Wrapped lines spread the space between words to fill the area. Last lines and lines ending at a newline keep natural spacing.</p>}
          {selection.alignment&&<p>Initial alignment was estimated from the selected lines. Check the preview.</p>}
          <div className="paragraph-dimensions">
            <label>Line spacing (× font size)<PreciseNumberInput disabled={dragging} min="0.6" max="3" step="any" value={lineSpacing} onChange={event=>{invalidate();setLineSpacing(event.target.value);setSpacingEdited(true);}}/></label>
            <button className="button" disabled={dragging||!spacingEdited} onClick={()=>{invalidate();setLineSpacing(spacingDraft(selection.lineSpacing??1.2));setSpacingEdited(false);}}>Reset line spacing</button>
          </div>
          <p>{selection.lineSpacing!=null?"Initial line spacing was estimated from the selected lines.":"Line spacing could not be inferred; the default is 1.2."} 1 is single spacing; 2 is double spacing. Widely spaced lines may need Adjust selection.</p>
          {!spacingValid&&<p className="existing-text-error">Use line spacing from 0.6 to 3 times the font size.</p>}
          <div className="paragraph-dimensions">
            <label>Left (pt)<PreciseNumberInput disabled={dragging} min="0" step="any" value={left} onChange={event=>{invalidate();setLeft(event.target.value);}}/></label>
            <label>Top (pt)<PreciseNumberInput disabled={dragging} min="0" step="any" value={top} onChange={event=>{invalidate();setTop(event.target.value);}}/></label>
            <label>Area width (pt)<PreciseNumberInput disabled={dragging} min="0.01" step="any" value={width} onChange={event=>{invalidate();setWidth(event.target.value);}}/></label>
            <label>Area height (pt)<PreciseNumberInput disabled={dragging} min="0.01" step="any" value={height} onChange={event=>{invalidate();setHeight(event.target.value);}}/></label>
          </div>
          <button className="button text-group-check" disabled={dragging||(left===String(selection.bounds.x)&&top===String(selection.bounds.y))} onClick={()=>{invalidate();setLeft(String(selection.bounds.x));setTop(String(selection.bounds.y));}}>Reset position</button>
          <p>Left and Top use the page shown in this preview, including rotation saved in the PDF. Extra workspace rotation is excluded. Text keeps its direction; the original font and color are used unless you change them. Size changes reflow within the area; newlines create hard breaks.</p>
          {!dimensionsValid&&<p className="existing-text-error">Keep the entire paragraph area inside the page.</p>}
        </>}
        {working && <p role="status">Checking the PDF…</p>}
        {error && <p className="existing-text-error" role="alert">{error}</p>}
      </div>
      {!canvas&&<ParagraphPagePreview crop={crop} adapter={adapter} sourceId={sourceId} pageIndex={pageIndex} pageWidth={pageWidth} pageHeight={pageHeight} overlays={overlays}
        runs={runs} selected={selected} selectable={choosing && !loading && !working} onToggle={toggle} forceOriginal={choosing}
        bounds={selection&&!choosing?(dimensionsValid?area:selection.bounds):undefined} candidateUrl={preview?.url}
        onBoundsChange={selection&&!choosing&&dimensionsValid&&!working?bounds=>{if(bounds.x===area.x&&bounds.y===area.y)return;invalidate();setLeft(String(bounds.x));setTop(String(bounds.y));}:undefined}
        onDraggingChange={setDragging}/>}
    </div>
    <footer className="dialog-actions"><button className="button" onClick={cancel}>Cancel</button><button className="button" disabled={!canPreview || working || dragging} onClick={()=>void makePreview()}>Preview paragraph</button><button className="button primary" disabled={!canPreview || choosing || !preview || working || dragging || !rasterReady} onClick={apply}>Apply paragraph</button></footer>
  </>;
  if(canvas)return <ParagraphWorkspace canvas={canvas} adapter={adapter} sourceId={sourceId} pageIndex={pageIndex} pageWidth={pageWidth} pageHeight={pageHeight} overlays={overlays} candidate={preview} onReady={canvasPreview.onReady} runs={runs} selected={selected} bounds={selection&&!choosing?(dimensionsValid?area:selection.bounds):undefined} choosing={choosing} loading={loading} enabled={!loading&&!working&&(choosing||canPreview)} dragging={dragging} picker={pickerView} onToggle={toggle} onChange={bounds=>{invalidate();setLeft(String(bounds.x));setTop(String(bounds.y));setWidth(String(bounds.width));setHeight(String(bounds.height));canvasPreview.queue();}} onGestureActive={setDragging}>{content}</ParagraphWorkspace>;
  if(pickerView)return pickerView;
  return <Modal title="Edit paragraph" className="existing-text-dialog paragraph-dialog" onClose={cancel}>{content}</Modal>;
}
