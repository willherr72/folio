export const propertyKeys=['title','author','subject','keywords'] as const;
export type PropertyKey=typeof propertyKeys[number];
export type PropertyDraft=Record<PropertyKey,string>;
export type DocumentMetadataEdit={mode:'remove'}|({mode:'update';clearExisting?:boolean}&Partial<PropertyDraft>);
export interface DocumentProperties extends PropertyDraft{creator:string;producer:string;creationDate:string;modificationDate:string;hasInfo:boolean;hasXmp:boolean;conflicts:PropertyKey[];multipleAuthors:boolean;error:string|null}
const bytes=(value:string)=>new TextEncoder().encode(value).length;
function text(value:unknown):value is string{return typeof value==='string'&&bytes(value)<=16384&&!/[\u0000-\u0008\u000b\u000c\u000e-\u001f\ud800-\udfff\ufffe\uffff]/u.test(value);}
function object(value:unknown):value is Record<string,unknown>{return !!value&&typeof value==='object'&&!Array.isArray(value);}
export function validateMetadata(value:unknown):asserts value is DocumentMetadataEdit{
 const bad=()=>{throw Error('Document properties must use valid text of at most 16 KiB per field.');};
 if(!object(value))return bad();
 if(value.mode==='remove'){if(Object.keys(value).some(key=>key!=='mode'))return bad();return;}
 if(value.mode!=='update'||Object.keys(value).some(key=>!['mode','clearExisting',...propertyKeys].includes(key)))return bad();
 if('clearExisting' in value&&typeof value.clearExisting!=='boolean')return bad();
 let count=0;for(const key of propertyKeys)if(key in value){if(!text(value[key]))return bad();count++;}if(!count)return bad();
}
export function propertiesDraft(source:PropertyDraft,edit?:DocumentMetadataEdit):PropertyDraft{
 if(edit)validateMetadata(edit);
 const clear=edit?.mode==='remove'||(edit?.mode==='update'&&edit.clearExisting);
 return Object.fromEntries(propertyKeys.map(key=>[key,edit?.mode==='update'&&edit[key]!==undefined?edit[key]:clear?'':source[key]])) as PropertyDraft;
}
export function metadataFromDraft(source:PropertyDraft,pending:DocumentMetadataEdit|undefined,draft:PropertyDraft,remove=false):DocumentMetadataEdit|undefined{
 if(remove)return {mode:'remove'};
 const initial=propertiesDraft(source,pending),fields:Partial<PropertyDraft>={};
 for(const key of propertyKeys){if(pending?.mode==='update'&&pending[key]!==undefined)fields[key]=pending[key];if(draft[key]!==initial[key]){if(!text(draft[key]))throw Error('Use valid text of at most 16 KiB per property.');fields[key]=draft[key];}}
 const clear=pending?.mode==='remove'||(pending?.mode==='update'&&pending.clearExisting===true);
 if(!Object.keys(fields).length)return clear?{mode:'remove'}:undefined;
 return {mode:'update',...(clear?{clearExisting:true}:{}),...fields};
}
export function metadataEqual(a:DocumentMetadataEdit|undefined,b:DocumentMetadataEdit|undefined):boolean{
 const key=(value:DocumentMetadataEdit|undefined)=>value?.mode==='update'?JSON.stringify(['update',value.clearExisting===true,...propertyKeys.map(k=>value[k]??null)]):value?.mode;
 return key(a)===key(b);
}
export function copyDocumentProperties(value:unknown):DocumentProperties{
 if(!object(value))throw Error('Document properties are unavailable.');
 const strings=[...propertyKeys,'creator','producer','creationDate','modificationDate'] as const;
 // Native inspection may expose old control characters verbatim; React renders
 // text safely. Only newly submitted edits have XML character constraints.
 if(strings.some(k=>typeof value[k]!=='string'||bytes(value[k] as string)>16384)||['hasInfo','hasXmp','multipleAuthors'].some(k=>typeof value[k]!=='boolean')||!Array.isArray(value.conflicts)||value.conflicts.length>4||value.conflicts.some(k=>!propertyKeys.includes(k))||(value.error!==null&&(typeof value.error!=='string'||bytes(value.error)>4096)))throw Error('The PDF service returned invalid document properties.');
 return {...Object.fromEntries(strings.map(key=>[key,value[key]])),hasInfo:value.hasInfo,hasXmp:value.hasXmp,multipleAuthors:value.multipleAuthors,conflicts:[...value.conflicts],error:value.error} as DocumentProperties;
}
