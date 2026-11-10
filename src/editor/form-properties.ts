import type {FolioAdapter} from './adapter';
import type {DocumentInfo} from './types';
import {discardOpenedDocument} from './discard-opened-document';
export interface FormProperties {expectedKey:string;name:string;fullName:string;label:string;readOnly:boolean;required:boolean;linkedWidgets:number;renameReason:string|null}
export interface FormPropertyDraft {name:string;label:string;readOnly:boolean;required:boolean}
export interface SetFormPropertiesRequest {pageIndex:number;annotationIndex:number;expectedKey:string;changes:Partial<FormPropertyDraft>}
export function propertiesDraft(p:FormProperties):FormPropertyDraft{return {name:p.name,label:p.label,readOnly:p.readOnly,required:p.required};}
export function formPropertiesRequest(p:FormProperties,d:FormPropertyDraft,pageIndex:number,annotationIndex:number):SetFormPropertiesRequest|null{
 const changes:Partial<FormPropertyDraft>={};const bytes=(s:string)=>new TextEncoder().encode(s).length;
 if(d.name!==p.name){
  if(p.renameReason)throw new Error(p.renameReason);
  if(!d.name.trim()||bytes(d.name)>1024||/[.\p{Cc}\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/u.test(d.name))throw new Error('Use a field name without periods or control characters, at most 1024 bytes.');
  changes.name=d.name;
 }
 if(d.label!==p.label){if(bytes(d.label)>4096||/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f\u061c\u200e\u200f\u202a-\u202e\u2066-\u2069]/u.test(d.label))throw new Error('Use a tooltip of at most 4096 bytes without control characters.');changes.label=d.label;}
 if(d.readOnly!==p.readOnly)changes.readOnly=d.readOnly;if(d.required!==p.required)changes.required=d.required;
 return Object.keys(changes).length?{pageIndex,annotationIndex,expectedKey:p.expectedKey,changes}:null;
}
export async function applyFormProperties(adapter:FolioAdapter,sourceId:string,request:SetFormPropertiesRequest,isBorrowed:(id:string)=>boolean,commit:(result:DocumentInfo)=>boolean):Promise<void>{
 if(!adapter.setFormProperties)throw new Error('Field properties require the desktop PDF service.');
 const result=await adapter.setFormProperties(sourceId,request);
 if(!result.id||isBorrowed(result.id))throw new Error('Updated field properties must have a new source version.');
 let adopted=false;
 try{adopted=commit(result);if(!adopted)throw new Error('The document changed. Reopen field properties and try again.');}
 finally{if(!adopted)await discardOpenedDocument(result,id=>adapter.closeDocument(id)).catch(()=>{});}
}
