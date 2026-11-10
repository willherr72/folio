import {formAuthorSelection} from './form-authoring';
import type {AnnotationRect,EditorDocument,FormField,FormFieldValue,PagePlan} from './types';

export type TextFieldAlignment='left'|'center'|'right';
export interface FormTextAppearance {styleKey:string;fontLabel:string;fontSize:number;color:string;colorSpace:string;alignment:TextFieldAlignment}
export interface TextFieldAppearanceChanges {fontSize?:number;color?:string;alignment?:TextFieldAlignment}
export interface SetFormTextAppearanceRequest {annotationIndex:number;expectedBounds:AnnotationRect;expectedValue:FormFieldValue;expectedStyleKey:string;changes:TextFieldAppearanceChanges}
export interface AppearanceDraft {size:string;autoFit:boolean;alignment:TextFieldAlignment;colorMode:'original'|'custom';color:string}
export const appearanceDraft=(style:FormTextAppearance):AppearanceDraft=>({size:String(style.fontSize),autoFit:style.fontSize===0,alignment:style.alignment,colorMode:'original',color:style.color});
export function fieldAppearanceRequest(document:EditorDocument,page:PagePlan,field:FormField,style:FormTextAppearance,draft:AppearanceDraft):SetFormTextAppearanceRequest {
 formAuthorSelection(document,page);
 if(field.kind!=='text'||field.password||!field.bounds||field.noRotate||field.visible===false)throw new Error('This field does not support text appearance changes.');
 const size=draft.autoFit?0:draft.size.trim()?Number(draft.size):NaN;
 if(!Number.isFinite(size)||(size!==style.fontSize&&!draft.autoFit&&(size<1||size>512)))throw new Error('Choose Auto fit or a font size from 1 to 512 pt.');
 if(!['left','center','right'].includes(draft.alignment))throw new Error('Choose Left, Center or Right alignment.');
 const changes:TextFieldAppearanceChanges={};
 if(size!==style.fontSize)changes.fontSize=size;
 if(draft.alignment!==style.alignment)changes.alignment=draft.alignment;
 if(draft.colorMode==='custom'){
  if(!/^#[0-9a-f]{6}$/i.test(draft.color))throw new Error('Enter an RGB color as #RRGGBB.');
  if(style.colorSpace!=='rgb'||draft.color.toLowerCase()!==style.color.toLowerCase())changes.color=draft.color.toLowerCase();
 }
 if(!Object.keys(changes).length)throw new Error('Change a text setting before previewing.');
 return {annotationIndex:field.annotationIndex,expectedBounds:field.bounds,expectedValue:field.value,expectedStyleKey:style.styleKey,changes};
}
