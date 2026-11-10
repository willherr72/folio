import {beforeEach,expect,it,vi} from 'vitest';
import {nativeAdapter} from '../src/editor/adapter';
import type {SetFormChoicesRequest} from '../src/editor/form-choices';
const ipc=vi.hoisted(()=>vi.fn());
vi.mock('@tauri-apps/api/core',()=>({invoke:ipc}));
beforeEach(()=>{ipc.mockReset();});

it('inspects the captured source annotation and keeps linked-page metadata',async()=>{
 const result={expectedKey:'source-key',widgets:[{pageIndex:2,annotationIndex:4},{pageIndex:5,annotationIndex:1}]};ipc.mockResolvedValue(result);
 expect(await nativeAdapter.inspectFormChoices!('source',2,4)).toBe(result);
 expect(ipc).toHaveBeenCalledWith('inspect_form_choices',{sourceId:'source',pageIndex:2,annotationIndex:4});
});

it('sends stable option identities and explicit value actions to a complete-source edit',async()=>{
 const request:SetFormChoicesRequest={pageIndex:2,annotationIndex:4,expectedKey:'source-key',edit:{
  options:[{sourceIndex:2,exportValue:'c',display:'Same'},{sourceIndex:null,exportValue:'new',display:'Same'},{sourceIndex:0,exportValue:'a',display:'Alpha'}],
  selection:{kind:'rows',value:[0,2]},defaults:{kind:'clear'},manualOrder:true,
 }};
 const candidate={id:'complete-candidate',name:'Updated form.pdf',pages:[{width:300,height:400},{width:400,height:300}]};ipc.mockResolvedValue(candidate);
 expect(await nativeAdapter.setFormChoices!('source',request)).toBe(candidate);
 expect(ipc).toHaveBeenCalledWith('set_form_choices',{sourceId:'source',request});
 expect(request.edit.options.map(o=>o.sourceIndex)).toEqual([2,null,0]);
});

it('preserves free text and Keep without translating them into legacy field-value payloads',async()=>{
 const request:SetFormChoicesRequest={pageIndex:0,annotationIndex:0,expectedKey:'key',edit:{options:[],selection:{kind:'text',value:'Free café'},defaults:{kind:'keep'},manualOrder:false}};
 ipc.mockResolvedValue({id:'candidate'});await nativeAdapter.setFormChoices!('source',request);
 expect(ipc).toHaveBeenCalledWith('set_form_choices',{sourceId:'source',request});
});

it('propagates native source-appearance refusal without returning or registering a candidate',async()=>{
 ipc.mockRejectedValue(new Error('Original appearance cannot be preserved'));
 const request:SetFormChoicesRequest={pageIndex:0,annotationIndex:0,expectedKey:'key',edit:{options:[],selection:{kind:'clear'},defaults:{kind:'clear'},manualOrder:false}};
 await expect(nativeAdapter.setFormChoices!('source',request)).rejects.toThrow('Original appearance cannot be preserved');
 expect(ipc).toHaveBeenCalledTimes(1);
});
