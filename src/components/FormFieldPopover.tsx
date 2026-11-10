import {useCallback,useLayoutEffect,useRef,useState,type MutableRefObject} from 'react';
import {createPortal} from 'react-dom';
import {placeFormEntry,type FormCanvasAnchor} from '../editor/form-canvas-entry';
import type {FormField,FormFieldValue} from '../editor/types';
import {FormFieldValueEditor} from './FormFieldValueEditor';
interface Props{anchor:FormCanvasAnchor;field:FormField;draft:FormFieldValue;disabled:boolean;busy:boolean;dirty:boolean;error:string|null;focusRevision:number;handledFocus:MutableRefObject<string|null>;entryRef:MutableRefObject<HTMLElement|null>;onFocused():void;onFocusFallback():void;onChange(value:FormFieldValue):void;onApply():void;onReset():void;onDismiss():void}
export function FormFieldPopover(props:Props){
 const {anchor,field,draft,disabled,busy,dirty,error}=props;
 const card=useRef<HTMLElement|null>(null),focused=useRef(false),callbacks=useRef(props);callbacks.current=props;
 const setCard=useCallback((node:HTMLElement|null)=>{if(node||props.entryRef.current===card.current)props.entryRef.current=node;card.current=node;},[props.entryRef]);
 const [position,setPosition]=useState<ReturnType<typeof placeFormEntry>>(null);
 const visible=position!==null;
 const focusKey=JSON.stringify([props.focusRevision,anchor.sourceId,anchor.pageId,anchor.pageIndex,anchor.annotationIndex]);
 useLayoutEffect(()=>{
  let frame:number|null=null;
  const viewport=anchor.element.closest('.document-viewport');
  const measure=()=>{
   frame=null;
   const next=anchor.element.isConnected?placeFormEntry(anchor.element.getBoundingClientRect(),viewport?.getBoundingClientRect()??{left:0,top:0,right:innerWidth,bottom:innerHeight},{width:innerWidth,height:innerHeight},card.current?.getBoundingClientRect().height||280):null;
   setPosition(previous=>JSON.stringify(previous)===JSON.stringify(next)?previous:next);
  };
  const schedule=()=>{if(frame===null)frame=requestAnimationFrame(measure);};
  measure();window.addEventListener('scroll',schedule,true);window.addEventListener('resize',schedule);
  const observer=typeof ResizeObserver==='undefined'?null:new ResizeObserver(schedule);
  for(const node of [anchor.element,anchor.element.ownerSVGElement,viewport,card.current])if(node)observer?.observe(node);
  return()=>{window.removeEventListener('scroll',schedule,true);window.removeEventListener('resize',schedule);observer?.disconnect();if(frame!==null)cancelAnimationFrame(frame);};
 },[anchor,visible]);
 useLayoutEffect(()=>{
  if(!visible||busy||props.handledFocus.current===focusKey)return;
  const node=card.current;if(!node)return;
  (node.querySelector<HTMLElement>('input:not(:disabled),select:not(:disabled),textarea:not(:disabled)')??node).focus({preventScroll:true});
  props.handledFocus.current=focusKey;props.onFocused();
 },[visible,busy,focusKey,props.handledFocus,draft]);
 useLayoutEffect(()=>{
  if(!visible)return;
  const node=card.current;
  return()=>{const restore=focused.current||node?.contains(document.activeElement);focused.current=false;if(restore)queueMicrotask(()=>{const active=document.activeElement;if(!active||active===document.body||!active.isConnected)callbacks.current.onFocusFallback();});};
 },[visible]);
 if(!position)return null;
 return createPortal(<section ref={setCard} className="form-field-popover form-fill-controls" role="region" aria-label="On-page form field" tabIndex={-1} style={{position:'fixed',...position}} onFocusCapture={()=>{focused.current=true;}} onBlurCapture={event=>{if(event.relatedTarget instanceof Node&&!event.currentTarget.contains(event.relatedTarget))focused.current=false;}} onPointerDown={event=>event.stopPropagation()} onKeyDown={event=>{
  event.stopPropagation();if(event.nativeEvent.isComposing||event.keyCode===229)return;
  if(event.key==='Escape'){event.preventDefault();if(!busy)props.onDismiss();}
  else if(event.key==='Enter'&&(event.ctrlKey||event.metaKey||(event.target instanceof HTMLInputElement&&['text','password'].includes(event.target.type)))){event.preventDefault();if(!disabled&&dirty)props.onApply();}
 }}>
  <p className="form-field-draft-label">Draft · Apply to update the PDF</p>
  <FormFieldValueEditor field={field} draft={draft} disabled={disabled} onChange={props.onChange} prefix="On page: "/>
  {error&&<p className="error-note">{error}</p>}
  {busy&&<p>Updating field and appearances…</p>}
  <div className="form-fill-buttons"><button className="button" disabled={busy||!dirty} onClick={props.onReset}>Reset field on page</button><button className="button primary" disabled={disabled||!dirty} onClick={props.onApply}>Apply field on page</button></div>
  <button className="button" disabled={busy} onClick={props.onDismiss}>Use Properties</button>
 </section>,document.body);
}
