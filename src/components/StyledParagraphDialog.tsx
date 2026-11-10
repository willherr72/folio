import {PreciseNumberInput} from "./PreciseNumberInput";
import {ParagraphWorkspace,useParagraphCanvasPreview} from "./ParagraphWorkspace";
import {paragraphRasterWidth,type ParagraphCanvasPresentation} from "../editor/paragraph-canvas";
import {paragraphReadAdapter,previewStyledParagraph,type ParagraphOccurrence,type ParagraphPreview} from "../editor/paragraph-transaction";
import {useEffect,useMemo,useId,useRef,useState} from "react";
import type {FolioAdapter} from "../editor/adapter";
import type {PreparedParagraphDraft} from "../editor/paragraph-draft";
import type {AnnotationRect,DocumentInfo,PageContentReplacement,EditableTextRun,Overlay,ParagraphAlignment,StyledParagraphSelection} from "../editor/types";
import {createStyledEditor,editStyledEditor,redoStyledEditor,styledEditorFontIds,styledEditorRequest,undoStyledEditor,type StyledEditorState,type StyledEditorTransaction} from "../editor/styled-paragraph-editor";
import {styledRangeStyle,styledText} from "../editor/styled-paragraph";
import {customTextError,holdCustomFont,type FontInfo} from "../editor/custom-fonts";
import {MAX_PARAGRAPH_PIECES,NO_PARAGRAPH_OVERLAYS,renderParagraphPage,validParagraphBounds} from "../editor/paragraph-preview";
import {Modal} from "./Modal";
import {ParagraphPagePreview} from "./ParagraphPagePreview";
import {StyledParagraphInput} from "./StyledParagraphInput";
import {FontPicker} from "./FontPicker";
import "./existing-text.css";

interface Props {
 initialDraft?:PreparedParagraphDraft;
 canvas?:ParagraphCanvasPresentation;
 occurrence?:ParagraphOccurrence;
 adapter:FolioAdapter;sourceId:string;pageIndex:number;pageWidth:number;pageHeight:number;
 overlays?:Overlay[];crop?:AnnotationRect;anchor:EditableTextRun;
 /** True transfers ownership of the preview document to the workspace. */
 onApply(document:DocumentInfo,replacement?:PageContentReplacement):boolean;onCancel():void;
}
type Candidate=ParagraphPreview&{url:string};
const message=(error:unknown)=>error instanceof Error?error.message:String(error);
const revoke=(url:string)=>{if(url.startsWith("blob:"))URL.revokeObjectURL(url);};
const release=(value:Candidate|null)=>{if(value){revoke(value.url);void value.release();}};

/** Local numeric drafts allow clearing a field without committing NaN to history. */
function NumberField({label,value,min,max,onChange,onValidity,resetKey,resetSignal=0,disabled=false}: {
 label:string;value:number|null;min:number;max?:number;onChange(value:number):void;
 onValidity(valid:boolean):void;resetKey:unknown;resetSignal?:number;disabled?:boolean;
}) {
 const [draft,setDraft]=useState(value===null?"":String(value));
 const validity=useRef(onValidity);validity.current=onValidity;
 useEffect(()=>{setDraft(value===null?"":String(value));validity.current(true);},[value,resetKey,resetSignal]);
 return <label>{label}<PreciseNumberInput min={min} max={max} step="any" disabled={disabled} value={draft} placeholder={value===null?"Mixed":undefined} onChange={event=>{
  const text=event.target.value;setDraft(text);const next=Number(text),valid=text.trim()!==""&&Number.isFinite(next)&&next>=min&&(max===undefined||next<=max);
  onValidity(valid);if(valid)onChange(next);
 }}/></label>;
}

export function StyledParagraphDialog({adapter:baseAdapter,occurrence,canvas,sourceId,pageIndex,pageWidth,pageHeight,crop,overlays=NO_PARAGRAPH_OVERLAYS,anchor,initialDraft,onApply,onCancel}:Props) {
 const flight=useRef(false),[nativeBusy,setNativeBusy]=useState(false);
 const adapter=useMemo(()=>paragraphReadAdapter(baseAdapter,occurrence,"styled"),[baseAdapter,occurrence]);
 const fitHelpId=useId();
 const [runs,setRuns]=useState<EditableTextRun[]>([]),[selected,setSelected]=useState([anchor.objectIndex]);
 const [editor,setEditor]=useState<StyledEditorState|null>(null),[loading,setLoading]=useState(true),[adjusting,setAdjusting]=useState(false);
 const [operationWorking,setWorking]=useState(false),[dragging,setDragging]=useState(false),[error,setError]=useState<string|null>(null);
 const [fitNotice,setFitNotice]=useState<string|null>(null);
 const [preview,setPreview]=useState<Candidate|null>(null),[picker,setPicker]=useState<{token:number;state:StyledEditorState}|null>(null);
 const [invalid,setInvalid]=useState<ReadonlySet<string>>(new Set());
 const [positionReset,setPositionReset]=useState(0);
 const current=useRef(editor),candidate=useRef<Candidate|null>(null),generation=useRef(0),alive=useRef(false),pending=useRef(false);
 const leases=useRef(new Map<string,()=>Promise<void>>()),fonts=useRef(new Map<string,FontInfo>());
 const inputContainer=useRef<HTMLDivElement>(null),pieceGroup=useRef<HTMLFieldSetElement>(null);
 const working=operationWorking||nativeBusy;
 const choosing=!editor||adjusting;
 const syncFonts=(next:StyledEditorState|null)=>{
  const needed=next?styledEditorFontIds(next):new Set<string>();
  for(const id of needed)if(!leases.current.has(id))leases.current.set(id,holdCustomFont(id));
  for(const [id,done] of leases.current)if(!needed.has(id)){leases.current.delete(id);void done();fonts.current.delete(id);}
 };
 const store=(next:StyledEditorState|null)=>{syncFonts(next);current.current=next;setEditor(next);};
 const stop=()=>{generation.current++;pending.current=false;setWorking(false);setError(null);setFitNotice(null);};
 const invalidate=()=>{stop();const owned=candidate.current;candidate.current=null;setPreview(null);release(owned);};
 const change=(next:StyledEditorState)=>{
  const previous=current.current;if(next===previous)return;
  // Merely moving the selection changes controls, but not the PDF candidate.
  if(!previous||JSON.stringify(styledEditorRequest(next))!==JSON.stringify(styledEditorRequest(previous)))invalidate();
  store(next);
 };
 const transact=(transaction:StyledEditorTransaction)=>{if(!current.current)return;try{change(editStyledEditor(current.current,transaction));}catch(error){setError(message(error));}};
 const install=(selection:StyledParagraphSelection)=>{
  const next=createStyledEditor(selection);invalidate();store(next);setSelected([...selection.objectIndices]);setInvalid(new Set());setAdjusting(false);setPicker(null);
 };
 const focusText=()=>requestAnimationFrame(()=>inputContainer.current?.querySelector<HTMLElement>('[role="textbox"]')?.focus());
 useEffect(()=>{
  alive.current=true;let active=true;const token=++generation.current;
  pending.current=false;setWorking(false);setLoading(true);setRuns([]);setSelected([anchor.objectIndex]);store(null);setAdjusting(false);setPreview(null);setError(null);setFitNotice(null);setPicker(null);setInvalid(new Set());
  const prepared=initialDraft?.kind==="styled"?initialDraft:null;
  void Promise.allSettled([adapter.listTextRuns!(sourceId,pageIndex),prepared?Promise.resolve(prepared.editor.original as StyledParagraphSelection):adapter.suggestStyledParagraph!(sourceId,pageIndex,anchor.objectIndex,anchor.text)]).then(([listed,suggested])=>{
   if(!active||token!==generation.current)return;
   if(listed.status==="fulfilled")setRuns(listed.value.runs);else setError(message(listed.reason));
   if(suggested.status==="fulfilled"){
    try{if(prepared){if(prepared.font)fonts.current.set(prepared.font.id,prepared.font);store(prepared.editor);setSelected([...prepared.editor.original.objectIndices]);}else install(suggested.value);}catch(error){setError(message(error));}
   }else setError(message(suggested.reason));
   setLoading(false);
  });
  return ()=>{active=false;alive.current=false;generation.current++;pending.current=false;const owned=candidate.current;candidate.current=null;release(owned);syncFonts(null);current.current=null;};
 },[adapter,sourceId,pageIndex,pageWidth,pageHeight,anchor.objectIndex,anchor.text,initialDraft]);
 // A new overlay list invalidates a comparison rendered with the old overlays.
 const previousOverlays=useRef(overlays);
 useEffect(()=>{if(previousOverlays.current!==overlays){previousOverlays.current=overlays;if(current.current)invalidate();}},[overlays]);
 const validity=(key:string,valid:boolean)=>setInvalid(old=>{if(old.has(key)===!valid)return old;const next=new Set(old);if(valid)next.delete(key);else next.add(key);return next;});
 const fieldValidity=(key:string,valid:boolean)=>{validity(key,valid);if(!valid)invalidate();};
 const beginAdjustment=()=>{if(!editor||pending.current||dragging)return;stop();setSelected([...editor.original.objectIndices]);setAdjusting(true);requestAnimationFrame(()=>pieceGroup.current?.focus());};
 const cancelAdjustment=()=>{if(!editor)return;stop();setSelected([...editor.original.objectIndices]);setAdjusting(false);focusText();};
 const toggle=(index:number)=>{if(!choosing||pending.current||dragging)return;if(selected.length>=MAX_PARAGRAPH_PIECES&&!selected.includes(index))return;stop();setSelected(old=>old.includes(index)?old.filter(id=>id!==index):[...old,index].sort((a,b)=>a-b));};
 const check=async()=>{
  if(pending.current||dragging||!choosing||!selected.length)return;
  if(editor&&JSON.stringify(selected)===JSON.stringify(editor.original.objectIndices)){cancelAdjustment();return;}
  if(selected.length>MAX_PARAGRAPH_PIECES){setError(`Select at most ${MAX_PARAGRAPH_PIECES} pieces when changing the paragraph.`);return;}
  stop();const token=generation.current;pending.current=true;setWorking(true);
  try{const result=await adapter.inspectStyledParagraph!(sourceId,pageIndex,[...selected]);if(!alive.current||token!==generation.current)return;install(result);focusText();}
  catch(error){if(alive.current&&token===generation.current)setError(message(error));}
  finally{if(alive.current&&token===generation.current){pending.current=false;setWorking(false);}}
 };
 const snapshot=editor?.present;
 const text=snapshot?styledText(snapshot.draft):"";
 const range=snapshot?.selection;
 const activeIds=new Set<number>();
 if(snapshot&&range){
  if(range.start===range.end)activeIds.add(snapshot.typingStyleId??styledRangeStyle(snapshot.draft,range.start,range.end)!);
  else {let offset=0;for(const run of snapshot.draft.runs){if(offset<range.end&&offset+run.text.length>range.start)activeIds.add(run.styleId);offset+=run.text.length;}}
 }
 const activeStyles=snapshot?.styles.filter(style=>activeIds.has(style.id))??[];
 const sizes=activeStyles.map(style=>style.fontSize??editor!.original.styles[style.sourceStyleId].fontSize);
 const colors=activeStyles.map(style=>style.color??editor!.original.styles[style.sourceStyleId].color);
 const names=activeStyles.map(style=>style.fontId?fonts.current.get(style.fontId)?.name??"Substitute font":editor!.original.styles[style.sourceStyleId].fontDisplayName??editor!.original.styles[style.sourceStyleId].fontName);
 const common=<T,>(values:T[]):T|null=>values.length&&values.every(value=>value===values[0])?values[0]:null;
 let fontError:string|null=null;
 if(snapshot)for(const run of snapshot.draft.runs){const style=snapshot.styles.find(s=>s.id===run.styleId)!;if(style.fontId){const info=fonts.current.get(style.fontId);fontError=info?customTextError(info,run.text):"The substitute font is unavailable. Choose the font again.";if(fontError)break;}}
 const dimensionsValid=!!snapshot&&validParagraphBounds(snapshot.bounds,pageWidth,pageHeight);
 const canPreview=!!editor&&!choosing&&dimensionsValid&&!invalid.size&&!fontError&&!!text.trim();
 const canOfferFit=!!adapter.fitStyledParagraph&&!!editor&&!choosing;
 const fieldError=!choosing&&editor?(invalid.size?"Complete the numeric fields with values in their allowed range.":!dimensionsValid?"Keep the entire paragraph area inside the page.":null):null;
 // A retained draft is hidden during selection adjustment. Report failures of
 // the active operation there; restore draft validation when editing resumes.
 const feedbackError=error||(!choosing?fontError:null)||fieldError;
 const fitArea=async()=>{
  if(pending.current||dragging||!canPreview||!editor||!adapter.fitStyledParagraph)return;
  const request=styledEditorRequest(editor);stop();const token=generation.current;pending.current=true;setWorking(true);
  const held=[...new Set(request.styleChanges?.flatMap(style=>style.fontId?[style.fontId]:[])??[])].map(id=>holdCustomFont(id));
  try{
   const bounds=await adapter.fitStyledParagraph(sourceId,pageIndex,request);
   if(!alive.current||token!==generation.current||!current.current)return;
   if(!validParagraphBounds(bounds,pageWidth,pageHeight))throw new Error("The fitted area is outside the page.");
   const changed=JSON.stringify(bounds)!==JSON.stringify(current.current.present.bounds);
   change(editStyledEditor(current.current,{type:"area",bounds,intent:"fit"}));
   setFitNotice(changed?"Area adjusted. Preview the paragraph before applying.":"The current area already fits the text.");focusText();
  }catch(error){if(alive.current&&token===generation.current)setError(message(error));}
  finally{await Promise.all(held.map(done=>done()));if(alive.current&&token===generation.current){pending.current=false;setWorking(false);}}
 };
 const makePreview=async():Promise<void>=>{
  if((canvas&&(flight.current||!!picker))||pending.current||dragging||!canPreview||!editor)return;
    canvasPreview.consume();
  const request=styledEditorRequest(editor);invalidate();const token=generation.current;pending.current=true;setWorking(true);if(canvas){flight.current=true;setNativeBusy(true);}
  const requestIds=new Set(request.styleChanges?.flatMap(style=>style.fontId?[style.fontId]:[])??[]);
  const held=[...requestIds].map(id=>holdCustomFont(id));let owned:ParagraphPreview|null=null,url:string|null=null;
  try{
   owned=await previewStyledParagraph(baseAdapter,occurrence,sourceId,pageIndex,request);if(!alive.current||token!==generation.current)return;
   url=canvas?"":await renderParagraphPage(adapter,owned.document.id,owned.pageIndex,pageWidth,pageHeight,overlays);if(!alive.current||token!==generation.current)return;
   const value={...owned,url};candidate.current=value;setPreview(value);owned=null;url=null;
  }catch(error){if(alive.current&&token===generation.current)setError(message(error));}
  finally{if(canvas){flight.current=false;if(alive.current)setNativeBusy(false);}for(const done of held)void done();if(url)revoke(url);if(owned)void owned.release();if(alive.current&&token===generation.current){pending.current=false;setWorking(false);}}
 };
 const canvasPreview=useParagraphCanvasPreview(canvas,preview,canPreview,working||dragging||!!picker,makePreview);
 const rasterReady=canvasPreview.ready(paragraphRasterWidth(pageWidth,canvas?.zoom??100));
 const apply=()=>{
  const owned=candidate.current;if(!owned||pending.current||dragging||choosing||!canPreview||!rasterReady)return;
  // Detach before the callback: a successful callback may synchronously unmount.
  candidate.current=null;
  try{if((owned.replacement?onApply(owned.document,owned.replacement):onApply(owned.document))){revoke(owned.url);setPreview(null);return;}if(alive.current){candidate.current=owned;setError("The page changed. Close this dialog and select the paragraph again.");}else release(owned);}
  catch(error){if(alive.current){candidate.current=owned;setError(message(error));}else release(owned);}
 };
 const cancel=()=>{invalidate();syncFonts(null);alive.current=false;onCancel();};
 const reset=()=>{if(editor)install(editor.original);};
 const patchBounds=(patch:Partial<AnnotationRect>)=>{if(snapshot)transact({type:"area",bounds:{...snapshot.bounds,...patch}});};
 const format=(patch:Parameters<typeof editStyledEditor>[1]&{type:"format"})=>transact(patch);
 let pickerView:React.ReactNode=null;
 if(picker&&editor){
  const {start,end}=picker.state.present.selection;
  pickerView=<FontPicker text={styledText(picker.state.present.draft).slice(start,end)} fontSize={common(sizes)??12} currentFontId={common(activeStyles.map(style=>style.fontId))??undefined} title="Choose a substitute font" description={start===end?"Choose a font for text you type next. Check the PDF preview after typing.":"Choose a font for the selected text. Check the PDF preview before applying."} onClose={()=>{setPicker(null);focusText();}} onChoose={info=>{
   if(!alive.current||picker.token!==generation.current||current.current!==picker.state)throw new Error("The paragraph changed. Choose the font again.");
   const next=editStyledEditor(picker.state,{type:"format",patch:{fontId:info.id}});
   fonts.current.set(info.id,info);change(next);setPicker(null);focusText();
  }}/>;
 }
 const content=<>
  <div className="paragraph-body"><div className="existing-text-fields">
   {loading?<p role="status">Finding paragraph…</p>:choosing?<>
    <p>Select the pieces of one paragraph. Fonts, sizes, and colors can differ.</p>
    {editor&&<p>Checking a different selection replaces this draft. Cancel adjustment to keep your edits and preview.</p>}
    <fieldset ref={pieceGroup} tabIndex={-1} className="text-group-pieces"><legend>Paragraph pieces · {selected.length} selected</legend>
     {runs.map(run=><label key={run.objectIndex}><input type="checkbox" checked={selected.includes(run.objectIndex)} disabled={working||dragging||(selected.length>=MAX_PARAGRAPH_PIECES&&!selected.includes(run.objectIndex))} onChange={()=>toggle(run.objectIndex)}/><span>{run.text.trim()?run.text:`Whitespace piece ${run.objectIndex+1}`}</span></label>)}
    </fieldset>
    <div className="paragraph-selection-actions"><button className="button" disabled={working||dragging||!selected.length} onClick={()=>void check()}>Check selection</button>{editor&&<button className="button" onClick={cancelAdjustment}>Cancel adjustment</button>}</div>
   </>:editor&&snapshot&&<>
    <div className="paragraph-editor-heading"><p>{editor.original.objectIndices.length} paragraph pieces</p><button className="button" disabled={working||dragging} onClick={beginAdjustment}>Adjust selection</button></div>
    <div ref={inputContainer}><StyledParagraphInput value={editor} onChange={change} onError={setError} disabled={dragging}/></div>
    <div className="paragraph-selection-actions"><button className="button" disabled={!editor.past.length||dragging} onClick={()=>change(undoStyledEditor(editor))}>Undo</button><button className="button" disabled={!editor.future.length||dragging} onClick={()=>change(redoStyledEditor(editor))}>Redo</button><button className="button" disabled={dragging} onClick={reset}>Reset paragraph</button></div>
    <p>{range!.start===range!.end?"Formatting applies to text you type next.":"Formatting applies to the selected text."} Check the PDF preview for exact font appearance.</p>
    <div className="existing-text-font-choice"><div><span>Font</span><strong>{common(names)??"Mixed"}</strong></div><button className="button" disabled={working||dragging} onClick={()=>setPicker({token:generation.current,state:editor})}>Choose substitute font…</button><button className="existing-text-original-button" disabled={dragging} onClick={()=>format({type:"format",patch:{fontId:null}})}>Use original font</button></div>
    <div className="paragraph-dimensions"><NumberField label="Font size (pt)" value={common(sizes)} min={1} max={300} disabled={dragging} resetKey={range} onValidity={valid=>fieldValidity("size",valid)} onChange={fontSize=>format({type:"format",patch:{fontSize}})}/><button className="button" disabled={dragging} onClick={()=>format({type:"format",patch:{fontSize:null}})}>Reset font size</button></div>
    <div className="paragraph-dimensions"><label>Text color<input type="color" disabled={dragging} value={common(colors)??"#000000"} onChange={event=>format({type:"format",patch:{color:event.target.value}})}/></label><span>{common(colors)??"Mixed colors"}</span><button className="button" disabled={dragging} onClick={()=>format({type:"format",patch:{color:null}})}>Use original color</button></div>
    <label>Paragraph alignment<select disabled={dragging} value={snapshot.alignment} onChange={event=>transact({type:"paragraph",alignment:event.target.value as ParagraphAlignment})}><option value="left">Left</option><option value="center">Center</option><option value="right">Right</option><option value="justify">Justify</option></select></label>
    <NumberField label="Line spacing (× font size)" value={snapshot.lineSpacing} min={0.6} max={3} resetKey={editor.original} disabled={dragging} onValidity={valid=>fieldValidity("spacing",valid)} onChange={lineSpacing=>transact({type:"paragraph",lineSpacing})}/>
    <div className="paragraph-dimensions">{([["x","Left (pt)"],["y","Top (pt)"],["width","Area width (pt)"],["height","Area height (pt)"]] as const).map(([key,label])=><NumberField key={key} label={label} value={snapshot.bounds[key]} min={key==="x"||key==="y"?0:0.01} resetKey={editor.original} resetSignal={key==="x"||key==="y"?positionReset:0} disabled={dragging} onValidity={valid=>fieldValidity(key,valid)} onChange={value=>patchBounds({[key]:value})}/>)}</div>
    <button className="button" disabled={dragging} onClick={()=>{transact({type:"reset-position"});setPositionReset(value=>value+1);}}>Reset position</button>
    <p>Left and Top use the page shown in the preview. Size changes reflow text within the area; newlines create hard breaks.</p>
   </>}
  </div>{!canvas&&<ParagraphPagePreview crop={crop} adapter={adapter} sourceId={sourceId} pageIndex={pageIndex} pageWidth={pageWidth} pageHeight={pageHeight} overlays={overlays} runs={runs} selected={selected} selectable={choosing&&!loading&&!working} forceOriginal={choosing} onToggle={toggle} bounds={!choosing?snapshot?.bounds:undefined} candidateUrl={preview?.url} onBoundsChange={!choosing&&dimensionsValid&&!working?bounds=>transact({type:"area",bounds}):undefined} onDraggingChange={setDragging}/>}</div>
  <footer className="dialog-actions paragraph-editor-footer">
   {(canOfferFit||working||fitNotice||feedbackError)&&<div className="paragraph-feedback" tabIndex={feedbackError?0:undefined}>
    {feedbackError&&<p role="alert" className="existing-text-error">{feedbackError}</p>}
    {working&&<p role="status">Checking the PDF…</p>}{fitNotice&&<p role="status">{fitNotice}</p>}
    {canOfferFit&&<p id={fitHelpId}>Try Fit area to text when added text does not fit. It keeps your fonts and sizes.</p>}
   </div>}
   <div className="paragraph-footer-actions">
    {canOfferFit&&<button className="button" aria-describedby={fitHelpId} disabled={!canPreview||working||dragging} onClick={()=>void fitArea()}>Fit area to text</button>}
    <div className="paragraph-commit-actions"><button className="button" onClick={cancel}>Cancel</button><button className="button" disabled={!canPreview||working||dragging} onClick={()=>void makePreview()}>Preview paragraph</button><button className="button primary" disabled={!canPreview||!preview||working||dragging||!rasterReady} onClick={apply}>Apply paragraph</button></div>
   </div>
  </footer>
 </>;
 if(canvas)return <ParagraphWorkspace canvas={canvas} adapter={adapter} sourceId={sourceId} pageIndex={pageIndex} pageWidth={pageWidth} pageHeight={pageHeight} overlays={overlays} candidate={preview} onReady={canvasPreview.onReady} runs={runs} selected={selected} bounds={!choosing?snapshot?.bounds:undefined} choosing={choosing} loading={loading} enabled={!loading&&!working&&(choosing||canPreview)} dragging={dragging} picker={pickerView} onToggle={toggle} onChange={bounds=>{transact({type:"area",bounds});canvasPreview.queue();}} onGestureActive={setDragging}>{content}</ParagraphWorkspace>;
 if(pickerView)return pickerView;
 return <Modal title="Edit paragraph" className="existing-text-dialog paragraph-dialog" onClose={cancel}>{content}</Modal>;
}
