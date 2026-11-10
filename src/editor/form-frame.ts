import type {AnnotationRect,EditorDocument,FormField,FormFieldValue,PagePlan} from './types';
import {formAuthorSelection} from './form-authoring';
export type FrameStyle='solid'|'dashed'|'underline'|'beveled'|'inset';
export interface FormFrameColor {colorSpace:string;components:number[];preview:string|null}
export interface FormFrameAppearance {styleKey:string;width:number;style:FrameStyle|null;border:FormFrameColor;background:FormFrameColor;supportedStyles:FrameStyle[]}
export type FrameColorChange={mode:'transparent'}|{mode:'rgb';color:string};
export interface FormFrameChanges {width?:number;style?:FrameStyle;border?:FrameColorChange;background?:FrameColorChange}
export interface SetFormFrameRequest {annotationIndex:number;expectedBounds:AnnotationRect;expectedValue:FormFieldValue;expectedStyleKey:string;changes:FormFrameChanges}
export interface FrameDraft {width:string;style:'original'|FrameStyle;borderMode:'original'|'transparent'|'rgb';borderColor:string;backgroundMode:'original'|'transparent'|'rgb';backgroundColor:string}
export const frameStyles:FrameStyle[]=['solid','dashed','underline','beveled','inset'];
export function validateFrameAppearance(style:FormFrameAppearance):void {
 const validColor=(c:FormFrameColor)=>{
  const count=({transparent:0,gray:1,rgb:3,cmyk:4} as Record<string,number>)[c?.colorSpace];
  return count!==undefined&&Array.isArray(c.components)&&c.components.length===count&&c.components.every(n=>Number.isFinite(n)&&n>=0&&n<=1)&&(count===0?c.preview===null:typeof c.preview==='string'&&/^#[0-9a-f]{6}$/i.test(c.preview));
 };
 if(!style||!/^([0-9a-f]{64})$/i.test(style.styleKey)||!Number.isFinite(style.width)||style.width<0||!Array.isArray(style.supportedStyles)||!style.supportedStyles.length||style.supportedStyles.some(s=>!frameStyles.includes(s))||new Set(style.supportedStyles).size!==style.supportedStyles.length||(style.style!==null&&!style.supportedStyles.includes(style.style))||!validColor(style.border)||!validColor(style.background))throw new Error('The field returned invalid border settings.');
}
export function frameDraft(style:FormFrameAppearance):FrameDraft {
 return {width:String(style.width),style:'original',borderMode:'original',borderColor:style.border.preview??'#000000',backgroundMode:'original',backgroundColor:style.background.preview??'#ffffff'};
}
export function fieldFrameRequest(document:EditorDocument,page:PagePlan,field:FormField,style:FormFrameAppearance,draft:FrameDraft):SetFormFrameRequest {
 formAuthorSelection(document,page);
 validateFrameAppearance(style);
 if(!['text','checkbox','radio','combo','list'].includes(field.kind)||!field.bounds||field.noRotate||field.visible===false)throw new Error('This field does not support border changes.');
 const width=draft.width.trim()?Number(draft.width):NaN;
 if(!Number.isFinite(width)||width<0||(width!==style.width&&width>32))throw new Error('Choose a border width from 0 to 32 pt.');
 const changes:FormFrameChanges={};
 if(width!==style.width)changes.width=width;
 if(draft.style!=='original'){
  if(!style.supportedStyles.includes(draft.style))throw new Error('Choose a supported border style.');
  if(draft.style!==style.style)changes.style=draft.style;
 }
 for(const key of ['border','background'] as const){
  const mode=draft[`${key}Mode`],color=draft[`${key}Color`];
  if(mode==='transparent'){if(style[key].colorSpace!=='transparent')changes[key]={mode};}
  else if(mode==='rgb'){
   if(!/^#[0-9a-f]{6}$/i.test(color))throw new Error('Enter an RGB color as #RRGGBB.');
   // Preview colors are rounded; never use them as proof of an unchanged palette.
   changes[key]={mode,color:color.toLowerCase()};
  }else if(mode!=='original')throw new Error('Choose Original, Transparent or Custom RGB.');
 }
 if(!Object.keys(changes).length)throw new Error('Change a border or background setting before previewing.');
 return {annotationIndex:field.annotationIndex,expectedBounds:field.bounds,expectedValue:field.value,expectedStyleKey:style.styleKey,changes};
}
