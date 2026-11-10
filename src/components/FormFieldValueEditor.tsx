import {useId} from 'react';
import type {FormField,FormFieldValue} from '../editor/types';
export function FormFieldValueEditor({field,draft,disabled,onChange,prefix=''}:{field:FormField;draft:FormFieldValue;disabled:boolean;onChange(value:FormFieldValue):void;prefix?:string}){
 const radioName=useId(),label=`${prefix}${field.label||field.name||'Unnamed field'}${field.required?' (required)':''}`;
 return <>
  {field.editableChoice&&<label>{label} choice<select disabled={disabled} value={draft.kind==='choice'?String(draft.indices[0]):'custom'} onChange={event=>onChange(event.target.value==='custom'?{kind:'text',text:''}:{kind:'choice',indices:[Number(event.target.value)]})}><option value="custom">Custom text</option>{field.options.map((option,i)=><option key={i} value={i}>{option}</option>)}</select></label>}
  {draft.kind==='text'&&<label>{label}{field.multiline?<textarea rows={5} disabled={disabled} value={draft.text} onChange={event=>onChange({kind:'text',text:event.target.value})}/>:<input autoComplete="off" type={field.password?'password':'text'} disabled={disabled} value={draft.text} onChange={event=>onChange({kind:'text',text:event.target.value})}/>}</label>}
  {draft.kind==='toggle'&&<label className="form-fill-toggle"><input type={field.kind==='radio'?'radio':'checkbox'} name={radioName} disabled={disabled} checked={draft.checked} onChange={event=>onChange({kind:'toggle',checked:event.target.checked})}/>{label}{field.kind==='radio'?` — ${field.options[0]||'Select this option'}`:''}</label>}
  {draft.kind==='choice'&&!field.editableChoice&&<label>{label}<select multiple={field.multiSelect} size={field.multiSelect?Math.min(7,Math.max(3,field.options.length)):undefined} disabled={disabled} value={field.multiSelect?draft.indices.map(String):String(draft.indices[0]??-1)} onChange={event=>onChange({kind:'choice',indices:Array.from(event.target.selectedOptions).map(option=>Number(option.value)).filter(i=>i>=0)})}>{!field.multiSelect&&<option value="-1" disabled={field.kind==='combo'}>No selection</option>}{field.options.map((option,i)=><option key={i} value={i}>{option}</option>)}</select></label>}
  {field.multiSelect&&<p>Use Ctrl or Shift to select multiple options.</p>}
  {field.reason&&<p>{field.reason}</p>}
 </>;
}
