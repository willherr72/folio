/** Source-bound native choice model. Row numbers refer to the original option order. */
export interface ChoiceOption {exportValue:string;display:string}
export interface ChoiceDraftOption extends ChoiceOption {sourceIndex:number|null}
export type ChoiceSelection={kind:'none'}|{kind:'rows';value:number[]}|{kind:'text';value:string};
export type ChoiceEditSelection={kind:'keep'}|{kind:'clear'}|{kind:'rows';value:number[]}|{kind:'text';value:string};
export interface ChoiceModel {
 options:ChoiceOption[];selection:ChoiceSelection;defaults:string[]|null;
 combo:boolean;editable:boolean;multiple:boolean;sorted:boolean;topIndex:number;
}
export interface FormChoices {
 expectedKey:string;name:string;widgets:{pageIndex:number;annotationIndex:number}[];model:ChoiceModel;
}
export interface ChoiceEdit {
 options:ChoiceDraftOption[];selection:ChoiceEditSelection;defaults:ChoiceEditSelection;manualOrder:boolean;
}
export interface SetFormChoicesRequest {
 pageIndex:number;annotationIndex:number;expectedKey:string;edit:ChoiceEdit;
}

export interface ChoiceRow extends ChoiceDraftOption {id:string}
export type ChoiceDraftSelection={kind:'keep'}|{kind:'clear'}|{kind:'rows';value:string[]}|{kind:'text';value:string};
export interface ChoiceDraft {rows:ChoiceRow[];selection:ChoiceDraftSelection;defaults:ChoiceDraftSelection;manualOrder:boolean}

const fail=(message:string):never=>{throw new Error(message);};
const encoder=new TextEncoder();
function payload(strings:Iterable<string>):void {
 let remaining=128*1024;
 for(const text of strings){
  if(typeof text!=='string')fail('Invalid choice text.');
  const size=encoder.encode(text).length;
  if(size>4096)fail('Each label or stored value must be at most 4096 UTF-8 bytes.');
  remaining-=size;if(remaining<0)fail('Choice text and values must fit within 128 KiB.');
 }
}
const optionText=(rows:ChoiceOption[])=>rows.flatMap(row=>[row.exportValue,row.display]);
const actionText=(action:ChoiceDraftSelection)=>action.kind==='text'?[action.value]:[];

export function createChoiceDraft(info:FormChoices):ChoiceDraft {
 const m=info?.model;
 if(!info||typeof info.expectedKey!=='string'||!info.expectedKey||info.expectedKey.length>128||typeof info.name!=='string'||!m||!Array.isArray(m.options)||m.options.length>2000||!Array.isArray(info.widgets)||!info.widgets.length||info.widgets.length>4000||info.widgets.some(w=>!Number.isSafeInteger(w.pageIndex)||w.pageIndex<0||!Number.isSafeInteger(w.annotationIndex)||w.annotationIndex<0))fail('The field returned invalid choice settings.');
 if(!m.selection||!['none','rows','text'].includes(m.selection.kind)||![m.combo,m.editable,m.multiple,m.sorted].every(v=>typeof v==='boolean')||m.defaults!==null&&(!Array.isArray(m.defaults)||m.defaults.length>2000))fail('The field returned invalid choice values.');
 if(m.selection.kind==='rows'&&(!Array.isArray(m.selection.value)||m.selection.value.length>2000||m.selection.value.some(i=>!Number.isInteger(i)||i<0||i>=m.options.length)))fail('The field returned invalid selected rows.');
 payload([...optionText(m.options),...(m.selection.kind==='text'?[m.selection.value]:[]),...(m.defaults??[])]);
 return {rows:m.options.map((o,i)=>({...o,id:`original-${i}`,sourceIndex:i})),selection:{kind:'keep'},defaults:{kind:'keep'},manualOrder:false};
}

export function newChoiceRow():ChoiceRow {
 return {id:`added-${crypto.randomUUID()}`,sourceIndex:null,display:'',exportValue:''};
}

/** Validate user decisions before native Preview. The native transaction remains
 * authoritative for raw PDF state (including absent-V default fallback), fonts,
 * permissions, appearance fidelity and the final resolved model. */
export function choiceRequest(info:FormChoices,draft:ChoiceDraft,pageIndex:number,annotationIndex:number):SetFormChoicesRequest|null {
 const source=info.model;
 if(draft.rows.length>2000)fail('Choice editing supports at most 2000 options.');
 payload([...optionText(draft.rows),...actionText(draft.selection),...actionText(draft.defaults)]);
 const rowPositions=new Map<string,number>(),originalPositions=new Map<number,number>();
 draft.rows.forEach((row,i)=>{
  if(!row.id||rowPositions.has(row.id))fail('An option row has a repeated or missing identity.');
  rowPositions.set(row.id,i);
  if(row.sourceIndex!==null){
   if(!Number.isInteger(row.sourceIndex)||row.sourceIndex<0||row.sourceIndex>=source.options.length||originalPositions.has(row.sourceIndex))fail('An original row is missing or appears more than once.');
   originalPositions.set(row.sourceIndex,i);
  }
 });
 const optionsChanged=draft.rows.length!==source.options.length||draft.rows.some((row,i)=>row.sourceIndex!==i||row.exportValue!==source.options[i]?.exportValue||row.display!==source.options[i]?.display);
 if(source.sorted&&optionsChanged&&!draft.manualOrder)fail('Enable manual order before changing sorted options.');
 const freeText=(text:string):string=>{
  if(!source.editable)fail('Free text is available only in an editable dropdown.');
  if(draft.rows.some(row=>row.exportValue===text))fail('This value matches an option; select its row explicitly.');
  return text;
 };
 const selectedRows=(ids:string[]):number[]=>{
  if(!ids.length)fail('Choose a row, or select Clear to remove the answer.');
  if(ids.length>2000)fail('Too many selected rows.');
  if(!source.multiple&&ids.length>1)fail('Choose at most one row for this field.');
  if(new Set(ids).size!==ids.length)fail('A selected row is repeated.');
  return ids.map(id=>rowPositions.get(id)??fail('A selected row was removed; choose or clear the answer.')).sort((a,b)=>a-b);
 };
 const action=(value:ChoiceDraftSelection):ChoiceEditSelection=>{
  if(value.kind==='rows')return {kind:'rows',value:selectedRows(value.value)};
  if(value.kind==='text')return {kind:'text',value:freeText(value.value)};
  return {kind:value.kind};
 };
 const selection=action(draft.selection),defaults=action(draft.defaults);
 let currentText:string[]=[];
 if(selection.kind==='keep'){
  if(source.selection.kind==='rows'&&source.selection.value.some(i=>!originalPositions.has(i)))fail('A selected row was removed; choose or clear the answer.');
  if(source.selection.kind==='text')currentText=[freeText(source.selection.value)];
 }else if(selection.kind==='text')currentText=[selection.value];
 let defaultText:string[]=[];
 if(defaults.kind==='keep'){
  defaultText=source.defaults??[];
  for(const value of defaultText){
   const matches=source.options.flatMap((o,i)=>o.exportValue===value?[i]:[]);
   if(!matches.length)freeText(value);
   else if(!matches.some(i=>{const at=originalPositions.get(i);return at!==undefined&&draft.rows[at].exportValue===value;}))fail('A default option changed or was removed; choose or clear the default.');
  }
 }else if(defaults.kind==='text')defaultText=[defaults.value];
 else if(defaults.kind==='rows')defaultText=[...new Set(defaults.value.map(i=>draft.rows[i].exportValue))];
 payload([...optionText(draft.rows),...currentText,...defaultText]);
 if(!optionsChanged&&selection.kind==='keep'&&defaults.kind==='keep'&&!(source.sorted&&draft.manualOrder))return null;
 return {pageIndex,annotationIndex,expectedKey:info.expectedKey,edit:{options:draft.rows.map(({sourceIndex,exportValue,display})=>({sourceIndex,exportValue,display})),selection,defaults,manualOrder:draft.manualOrder}};
}
