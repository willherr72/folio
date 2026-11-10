import {expect,it} from 'vitest';
import {createChoiceDraft,choiceRequest,newChoiceRow,type FormChoices} from '../src/editor/form-choices';

function info():FormChoices{return {expectedKey:'choice-key',name:'Answer',widgets:[{pageIndex:0,annotationIndex:2},{pageIndex:1,annotationIndex:0}],model:{options:[{exportValue:'a',display:'Same'},{exportValue:'b',display:'Same'},{exportValue:'c',display:'Third'}],selection:{kind:'rows',value:[1]},defaults:['a'],combo:true,editable:false,multiple:false,sorted:false,topIndex:0}};}
it('starts with independent stable rows and explicit Keep actions, and resets exactly',()=>{
 const source=info(),draft=createChoiceDraft(source),reset=createChoiceDraft(source);
 expect(choiceRequest(source,draft,0,2)).toBeNull();expect(draft.selection).toEqual({kind:'keep'});expect(draft.defaults).toEqual({kind:'keep'});
 expect(new Set(draft.rows.map(r=>r.id)).size).toBe(3);draft.rows[1].display='Edited';expect(source.model.options[1].display).toBe('Same');
 expect(reset).toEqual(createChoiceDraft(source));expect(choiceRequest(source,draft,0,2)?.edit.options[1]).toEqual({sourceIndex:1,exportValue:'b',display:'Edited'});
});
it('keeps selected identities across duplicate labels, export changes and reordering',()=>{
 const source=info(),draft=createChoiceDraft(source),selected=draft.rows[1].id;
 draft.selection={kind:'rows',value:[selected]};draft.rows[1].exportValue='new-value';draft.rows.reverse();
 const request=choiceRequest(source,draft,0,2)!;expect(request.edit.selection).toEqual({kind:'rows',value:[1]});expect(request.edit.options.map(r=>r.sourceIndex)).toEqual([2,1,0]);
 expect(request.edit.defaults).toEqual({kind:'keep'});expect(request.expectedKey).toBe('choice-key');
});
it('requires an explicit answer/default decision when an in-use row disappears',()=>{
 const source=info(),draft=createChoiceDraft(source);draft.rows.splice(1,1);
 expect(()=>choiceRequest(source,draft,0,2)).toThrow(/selected row/);draft.selection={kind:'clear'};
 expect(choiceRequest(source,draft,0,2)?.edit.selection).toEqual({kind:'clear'});
 draft.rows.shift();expect(()=>choiceRequest(source,draft,0,2)).toThrow(/default/);draft.defaults={kind:'clear'};
 expect(choiceRequest(source,draft,0,2)).not.toBeNull();
});
it.each(['selection','defaults'] as const)('requires an actual replacement row or explicit Clear for %s',which=>{
 const source=info(),draft=createChoiceDraft(source);draft.rows.splice(which==='selection'?1:0,1);
 draft[which]={kind:'rows',value:[]};expect(()=>choiceRequest(source,draft,0,2)).toThrow(/choose.*row.*Clear/i);
 draft[which]={kind:'rows',value:[draft.rows[0].id]};expect(choiceRequest(source,draft,0,2)?.edit[which]).toEqual({kind:'rows',value:[0]});
 draft[which]={kind:'clear'};expect(choiceRequest(source,draft,0,2)?.edit[which]).toEqual({kind:'clear'});
});
it('does not silently keep a default after changing the last matching original export',()=>{
 const source=info();source.model.options[1].exportValue='a';const draft=createChoiceDraft(source);
 draft.rows.shift();draft.selection={kind:'clear'};expect(()=>choiceRequest(source,draft,0,2)).not.toThrow();
 draft.rows[0].exportValue='replacement';expect(()=>choiceRequest(source,draft,0,2)).toThrow(/default/);
 draft.defaults={kind:'rows',value:[draft.rows[0].id]};expect(choiceRequest(source,draft,0,2)?.edit.defaults).toEqual({kind:'rows',value:[0]});
});
it('keeps free text separate and refuses newly ambiguous option values',()=>{
 const source=info();source.model.editable=true;source.model.selection={kind:'text',value:'Custom'};
 const draft=createChoiceDraft(source);draft.rows.push({...newChoiceRow(),exportValue:'Custom',display:'Custom label'});
 expect(()=>choiceRequest(source,draft,0,2)).toThrow(/matches an option/);
 draft.selection={kind:'rows',value:[draft.rows[3].id]};expect(choiceRequest(source,draft,0,2)?.edit.selection).toEqual({kind:'rows',value:[3]});
 draft.selection={kind:'text',value:'Another'};expect(choiceRequest(source,draft,0,2)?.edit.selection).toEqual({kind:'text',value:'Another'});
 source.model.editable=false;expect(()=>choiceRequest(source,draft,0,2)).toThrow(/editable dropdown/);
});
it('requires manual order for changing sorted fields',()=>{
 const source=info();source.model.sorted=true;const draft=createChoiceDraft(source);draft.rows[0].display='Changed';
 expect(()=>choiceRequest(source,draft,0,2)).toThrow(/manual order/);draft.manualOrder=true;
 expect(choiceRequest(source,draft,0,2)?.edit.manualOrder).toBe(true);
});
it('supports zero and 2000 options, charges UTF-8 bytes and rejects aggregate overflow',()=>{
 const source=info();source.model.selection={kind:'none'};source.model.defaults=null;const draft=createChoiceDraft(source);draft.rows=[];
 expect(choiceRequest(source,draft,0,2)?.edit.options).toEqual([]);
 draft.rows=Array.from({length:2000},(_,i)=>({...newChoiceRow(),display:`Row ${i}`,exportValue:`${i}`}));expect(choiceRequest(source,draft,0,2)?.edit.options).toHaveLength(2000);
 draft.rows.push(newChoiceRow());expect(()=>choiceRequest(source,draft,0,2)).toThrow(/2000/);draft.rows.pop();
 draft.rows[0].display='é'.repeat(2048);expect(()=>choiceRequest(source,draft,0,2)).not.toThrow();draft.rows[0].display+='é';expect(()=>choiceRequest(source,draft,0,2)).toThrow(/4096/);
 draft.rows=Array.from({length:40},()=>({...newChoiceRow(),display:'x'.repeat(4096)}));expect(()=>choiceRequest(source,draft,0,2)).toThrow(/128/);
});
it('bounds resolved retained defaults as well as new draft text',()=>{
 const source=info();source.model.editable=true;source.model.defaults=['d'.repeat(4096)];source.model.selection={kind:'none'};
 const draft=createChoiceDraft(source);draft.rows=Array.from({length:32},()=>({...newChoiceRow(),display:'x'.repeat(4096)}));
 expect(()=>choiceRequest(source,draft,0,2)).toThrow(/128/);draft.defaults={kind:'clear'};expect(()=>choiceRequest(source,draft,0,2)).not.toThrow();
});
it('refuses duplicate row identities, reused source rows and stale selected rows',()=>{
 const source=info(),draft=createChoiceDraft(source);draft.rows[1].id=draft.rows[0].id;expect(()=>choiceRequest(source,draft,0,2)).toThrow(/identity/);
 const duplicate=createChoiceDraft(source);duplicate.rows[1].sourceIndex=0;expect(()=>choiceRequest(source,duplicate,0,2)).toThrow(/original row/);
 const missing=createChoiceDraft(source);missing.selection={kind:'rows',value:['missing']};expect(()=>choiceRequest(source,missing,0,2)).toThrow(/selected row/);
});
it('sorts multiselect indices after reordering and rejects duplicate/single-field multiple selection',()=>{
 const source=info();source.model.combo=false;source.model.multiple=true;const draft=createChoiceDraft(source);draft.selection={kind:'rows',value:[draft.rows[2].id,draft.rows[0].id]};
 expect(choiceRequest(source,draft,0,2)?.edit.selection).toEqual({kind:'rows',value:[0,2]});
 source.model.multiple=false;expect(()=>choiceRequest(source,draft,0,2)).toThrow(/one row/);source.model.multiple=true;
 draft.selection={kind:'rows',value:[draft.rows[0].id,draft.rows[0].id]};expect(()=>choiceRequest(source,draft,0,2)).toThrow(/repeated/);
});
