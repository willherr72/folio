import type {FolioAdapter} from './adapter';
import type {DocumentInfo} from './types';
import {discardOpenedDocument} from './discard-opened-document';

export interface FormRemovalInfo {
 expectedKey:string;fullName:string;linkedWidgets:number;pageIndices:number[];removalReason:string|null;
}
export interface RemoveFormFieldRequest {pageIndex:number;annotationIndex:number;expectedKey:string}

export async function applyFormRemoval(adapter:FolioAdapter,sourceId:string,request:RemoveFormFieldRequest,isBorrowed:(id:string)=>boolean,commit:(result:DocumentInfo)=>boolean):Promise<void> {
 if(!adapter.removeFormField)throw new Error('Field removal requires the desktop PDF service.');
 const result=await adapter.removeFormField(sourceId,request);
 if(!result.id||result.id===sourceId||isBorrowed(result.id))throw new Error('The updated form must have a new source version.');
 let adopted=false;
 try{adopted=commit(result);if(!adopted)throw new Error('The document changed. Reopen field removal and try again.');}
 finally{if(!adopted)await discardOpenedDocument(result,id=>adapter.closeDocument(id)).catch(()=>{});}
}
