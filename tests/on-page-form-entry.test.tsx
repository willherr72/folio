import {act,cleanup,fireEvent,render,screen,waitFor,within} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {FormFillEditor} from '../src/components/FormFillEditor';
import {PropertiesPanel} from '../src/components/PropertiesPanel';
import {nativeAdapter} from '../src/editor/adapter';
import {formAnchorIdentity,type FormCanvasAnchor} from '../src/editor/form-canvas-entry';
import type {FormField,PagePlan} from '../src/editor/types';
const page:PagePlan={id:'p',sourceId:'s',pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
const field:FormField={annotationIndex:2,name:'name',label:'Full name',kind:'text',value:{kind:'text',text:'Original'},options:[],selectedIndices:[],readOnly:false,required:false,multiline:false,password:false,multiSelect:false,editableChoice:false,reason:null,visible:true,bounds:{x:20,y:30,width:150,height:20}};
let host:HTMLElement,element:SVGRectElement,rect:DOMRect;
beforeEach(()=>{
 host=document.createElement('main');host.className='document-viewport';document.body.append(host);host.innerHTML='<svg><rect/></svg>';element=host.querySelector('rect')!;
 rect={left:150,top:100,right:350,bottom:125,width:200,height:25,x:150,y:100,toJSON(){}};
 vi.spyOn(element,'getBoundingClientRect').mockImplementation(()=>rect);vi.spyOn(host,'getBoundingClientRect').mockReturnValue({left:0,top:0,right:700,bottom:600,width:700,height:600,x:0,y:0,toJSON(){}});
});
afterEach(()=>{cleanup();host.remove();vi.restoreAllMocks();});
function setup(value=field){
 const adapter={...nativeAdapter,listFormFields:vi.fn().mockResolvedValue([value])};
 const anchor:FormCanvasAnchor={...formAnchorIdentity(adapter,page,value),element};
 const props={page,adapter,pageNumber:1,onApply:vi.fn().mockResolvedValue(undefined),onClose:vi.fn(),preview:false,selectedAnnotationIndex:2,selectionRevision:1,canvasAnchor:anchor};
 return {props,...render(<FormFillEditor {...props}/>)};
}
it('shares on-page typing and Reset with Properties, then applies once',async()=>{
 const {props}=setup();const card=await screen.findByRole('region',{name:'On-page form field'});const inline=within(card).getByRole('textbox',{name:'On page: Full name'});expect(inline).toHaveFocus();
 fireEvent.change(inline,{target:{value:'Draft'}});expect(screen.getByRole('textbox',{name:'Full name'})).toHaveValue('Draft');
 fireEvent.click(within(card).getByRole('button',{name:'Reset field on page'}));expect(inline).toHaveValue('Original');
 fireEvent.change(screen.getByRole('textbox',{name:'Full name'}),{target:{value:'Changed in Properties'}});expect(inline).toHaveValue('Changed in Properties');
 let finish!:()=>void;props.onApply.mockImplementation(()=>new Promise<void>(resolve=>{finish=resolve;}));
 fireEvent.keyDown(inline,{key:'Enter'});fireEvent.keyDown(inline,{key:'Enter'});expect(props.onApply).toHaveBeenCalledOnce();expect(props.onApply).toHaveBeenCalledWith(field,{kind:'text',text:'Changed in Properties'});
 expect(within(card).getByRole('button',{name:'Use Properties'})).toBeDisabled();await act(async()=>finish());
});
it('retains a draft on Escape and reopens it on explicit same-field selection',async()=>{
 const view=setup();const inline=await screen.findByRole('textbox',{name:'On page: Full name'});fireEvent.change(inline,{target:{value:'Keep'}});fireEvent.keyDown(inline,{key:'Escape'});
 expect(screen.queryByRole('region',{name:'On-page form field'})).toBeNull();expect(screen.getByRole('textbox',{name:'Full name'})).toHaveValue('Keep');expect(screen.getByRole('textbox',{name:'Full name'})).toHaveFocus();
 view.rerender(<FormFillEditor {...view.props} selectionRevision={2}/>);expect(await screen.findByRole('textbox',{name:'On page: Full name'})).toHaveValue('Keep');expect(screen.getByRole('textbox',{name:'On page: Full name'})).toHaveFocus();
});
it('keeps focus on the card when the already-visible field is selected again',async()=>{
 const view=setup();const inline=await screen.findByRole('textbox',{name:'On page: Full name'});fireEvent.change(inline,{target:{value:'Still here'}});
 view.rerender(<FormFillEditor {...view.props} selectionRevision={2}/>);expect(inline).toHaveFocus();expect(inline).toHaveValue('Still here');expect(screen.getByRole('textbox',{name:'Full name'})).toHaveValue('Still here');
});
it('does not apply IME Enter and retains the draft after a native failure',async()=>{
 const {props}=setup();props.onApply.mockRejectedValue(Error('Native update failed'));const inline=await screen.findByRole('textbox',{name:'On page: Full name'});fireEvent.change(inline,{target:{value:'Retry'}});
 fireEvent.keyDown(inline,{key:'Enter',isComposing:true});expect(props.onApply).not.toHaveBeenCalled();fireEvent.keyDown(inline,{key:'Enter'});await screen.findByRole('alert');
 expect(inline).toHaveValue('Retry');expect(screen.getByRole('button',{name:'Apply field on page'})).toBeEnabled();
});
it('hides with the target while retaining draft and does not steal focus when it returns',async()=>{
 setup();const inline=await screen.findByRole('textbox',{name:'On page: Full name'});fireEvent.change(inline,{target:{value:'Keep while scrolling'}});
 rect={...rect,top:-100,bottom:-75};fireEvent.scroll(window);await waitFor(()=>expect(screen.queryByRole('region',{name:'On-page form field'})).toBeNull());
 const properties=screen.getByRole('textbox',{name:'Full name'});expect(properties).toHaveValue('Keep while scrolling');expect(properties).toHaveFocus();properties.focus();rect={...rect,top:100,bottom:125};fireEvent.scroll(window);
 expect(await screen.findByRole('textbox',{name:'On page: Full name'})).toHaveValue('Keep while scrolling');expect(properties).toHaveFocus();
});
it('rejects an anchor from another occurrence and keeps password values masked',async()=>{
 const view=setup({...field,password:true});await screen.findByLabelText('On page: Full name');expect(screen.getByLabelText('On page: Full name')).toHaveAttribute('type','password');
 view.rerender(<FormFillEditor {...view.props} canvasAnchor={{...view.props.canvasAnchor,pageId:'another'}}/>);expect(screen.queryByRole('region',{name:'On-page form field'})).toBeNull();expect(screen.getByLabelText('Full name')).toHaveAttribute('type','password');
});
it('uses Ctrl+Enter for multiline values and preserves normal newlines',async()=>{
 const {props}=setup({...field,multiline:true});const inline=await screen.findByRole('textbox',{name:'On page: Full name'});fireEvent.change(inline,{target:{value:'One\nTwo'}});fireEvent.keyDown(inline,{key:'Enter'});expect(props.onApply).not.toHaveBeenCalled();fireEvent.keyDown(inline,{key:'Enter',ctrlKey:true});await waitFor(()=>expect(props.onApply).toHaveBeenCalledOnce());
});
it('does not reopen collapsed Properties merely because a portal input receives focus',async()=>{
 const adapter={...nativeAdapter,listFormFields:vi.fn().mockResolvedValue([field])};const anchor={...formAnchorIdentity(adapter,page,field),element};
 render(<PropertiesPanel contextKey="form" autoExpand><FormFillEditor page={page} adapter={adapter} pageNumber={1} onApply={vi.fn()} onClose={vi.fn()} preview={false} selectedAnnotationIndex={2} selectionRevision={1} canvasAnchor={anchor}/></PropertiesPanel>);
 const inline=await screen.findByRole('textbox',{name:'On page: Full name'});const toggle=screen.getByRole('button',{name:'Properties'});fireEvent.click(toggle);expect(toggle).toHaveAttribute('aria-expanded','false');
 act(()=>{toggle.focus();inline.focus();});expect(toggle).toHaveAttribute('aria-expanded','false');
});
it.each(['Escape','Use Properties','scroll'])('expands Properties before focus handoff after %s',async action=>{
 // JSDOM permits focus under display:none; emulate the browser visibility boundary.
 const focus=HTMLElement.prototype.focus;vi.spyOn(HTMLElement.prototype,'focus').mockImplementation(function(this:HTMLElement,options){if(!this.closest('.properties.is-collapsed .properties-body'))focus.call(this,options);});
 const adapter={...nativeAdapter,listFormFields:vi.fn().mockResolvedValue([field])};const anchor={...formAnchorIdentity(adapter,page,field),element};
 render(<PropertiesPanel contextKey="form" autoExpand><FormFillEditor page={page} adapter={adapter} pageNumber={1} onApply={vi.fn()} onClose={vi.fn()} preview={false} selectedAnnotationIndex={2} selectionRevision={1} canvasAnchor={anchor}/></PropertiesPanel>);
 const inline=await screen.findByRole('textbox',{name:'On page: Full name'});fireEvent.change(inline,{target:{value:'Preserved'}});const toggle=screen.getByRole('button',{name:'Properties'});fireEvent.click(toggle);act(()=>{toggle.focus();inline.focus();});expect(toggle).toHaveAttribute('aria-expanded','false');
 if(action==='Escape')fireEvent.keyDown(inline,{key:'Escape'});else if(action==='Use Properties')fireEvent.click(screen.getByRole('button',{name:'Use Properties'}));else{rect={...rect,top:-100,bottom:-75};fireEvent.scroll(window);}
 await waitFor(()=>expect(toggle).toHaveAttribute('aria-expanded','true'));expect(screen.getByRole('textbox',{name:'Full name'})).toHaveValue('Preserved');expect(screen.getByRole('textbox',{name:'Full name'})).toHaveFocus();
});
it('returns focus to on-page entry after source refresh from Apply',async()=>{
 const view=setup();const inline=await screen.findByRole('textbox',{name:'On page: Full name'});fireEvent.change(inline,{target:{value:'Saved value'}});fireEvent.click(screen.getByRole('button',{name:'Apply field on page'}));await waitFor(()=>expect(view.props.onApply).toHaveBeenCalledOnce());
 view.props.adapter.listFormFields.mockResolvedValue([{...field,value:{kind:'text',text:'Saved value'}}]);const next={...page,sourceId:'filled'};
 view.rerender(<FormFillEditor {...view.props} page={next} canvasAnchor={{...formAnchorIdentity(view.props.adapter,next,field),element}}/>);
 await waitFor(()=>expect(screen.getByRole('textbox',{name:'On page: Full name'})).toHaveValue('Saved value'));expect(screen.getByRole('textbox',{name:'On page: Full name'})).toHaveFocus();
});
it('does not expand Properties or steal deliberate external focus when the card hides',async()=>{
 const adapter={...nativeAdapter,listFormFields:vi.fn().mockResolvedValue([field])};const anchor={...formAnchorIdentity(adapter,page,field),element};
 render(<><button>Other action</button><PropertiesPanel contextKey="form" autoExpand><FormFillEditor page={page} adapter={adapter} pageNumber={1} onApply={vi.fn()} onClose={vi.fn()} preview={false} selectedAnnotationIndex={2} selectionRevision={1} canvasAnchor={anchor}/></PropertiesPanel></>);
 const inline=await screen.findByRole('textbox',{name:'On page: Full name'});const toggle=screen.getByRole('button',{name:'Properties'});fireEvent.click(toggle);const other=screen.getByRole('button',{name:'Other action'});act(()=>{inline.focus();other.focus();});
 rect={...rect,top:-100,bottom:-75};fireEvent.scroll(window);await waitFor(()=>expect(screen.queryByRole('region',{name:'On-page form field'})).toBeNull());expect(other).toHaveFocus();expect(toggle).toHaveAttribute('aria-expanded','false');
});
it.each(['checkbox','radio'])('shares %s state without cross-surface radio grouping',async kind=>{
 setup({...field,kind,value:{kind:'toggle',checked:false},options:kind==='radio'?['Yes']:[]});const name=kind==='radio'?'On page: Full name — Yes':'On page: Full name';const inline=await screen.findByRole(kind,{name});fireEvent.click(inline);
 expect(inline).toBeChecked();expect(screen.getByRole(kind,{name:kind==='radio'?'Full name — Yes':'Full name'})).toBeChecked();fireEvent.click(screen.getByRole('button',{name:'Reset field on page'}));expect(inline).not.toBeChecked();
});
it('retains choice indices and editable-combo custom text across both surfaces',async()=>{
 setup({...field,kind:'combo',editableChoice:true,options:['Duplicate','Duplicate'],selectedIndices:[0],value:{kind:'choice',indices:[0]}});
 const choice=await screen.findByRole('combobox',{name:'On page: Full name choice'});fireEvent.change(choice,{target:{value:'1'}});expect(screen.getByRole('combobox',{name:'Full name choice'})).toHaveValue('1');
 fireEvent.change(choice,{target:{value:'custom'}});fireEvent.change(screen.getByRole('textbox',{name:'On page: Full name'}),{target:{value:'Custom'}});expect(screen.getByRole('textbox',{name:'Full name'})).toHaveValue('Custom');
});
it('keeps unsupported fields explained and disabled',async()=>{
 setup({...field,readOnly:true,reason:'This field is read-only.'});const inline=await screen.findByRole('textbox',{name:'On page: Full name'});expect(inline).toBeDisabled();expect(within(screen.getByRole('region',{name:'On-page form field'})).getByText('This field is read-only.')).toBeInTheDocument();expect(screen.getByRole('button',{name:'Apply field on page'})).toBeDisabled();
});
