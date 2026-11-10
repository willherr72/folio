import {it,expect,vi,afterEach} from 'vitest';
import {render,screen,fireEvent,cleanup,waitFor,act} from '@testing-library/react';
import {DocumentPropertiesDialog} from '../src/components/DocumentPropertiesDialog';
import type {DocumentProperties} from '../src/editor/document-properties';
afterEach(cleanup);
const source:DocumentProperties={title:'Original',author:'Author',subject:'Subject',keywords:'Keywords',creator:'Creator',producer:'Producer',creationDate:'2026',modificationDate:'2026',hasInfo:true,hasXmp:true,conflicts:[],multipleAuthors:false,error:null};
it.each([[false,false],[true,false],[false,true],[true,true]])('shows source presence independently of empty standard fields (%s, %s)',async(hasInfo,hasXmp)=>{
 setup({adapter:{inspectDocumentProperties:vi.fn().mockResolvedValue({...source,title:'',author:'',subject:'',keywords:'',creator:'',producer:'',creationDate:'',modificationDate:'',hasInfo,hasXmp})}});
 await screen.findByLabelText('Title');
 expect(screen.getByText('Source PDF information').nextElementSibling).toHaveTextContent(hasInfo?'Present':'Not present');
 expect(screen.getByText('Source XMP metadata').nextElementSibling).toHaveTextContent(hasXmp?'Present':'Not present');
});
it.each([{mode:'remove' as const},{mode:'update' as const,clearExisting:true,title:'Public title'}])('keeps source presence distinct from a cleared draft ($mode)',async metadata=>{
 setup({metadata});await screen.findByLabelText('Title');
 expect(screen.getByText('Source PDF information').nextElementSibling).toHaveTextContent('Present');
 expect(screen.getByText('Source XMP metadata').nextElementSibling).toHaveTextContent('Present');
 expect(screen.getByText(/Presence describes the source PDF before draft changes/)).toBeVisible();
});
function setup(extra:Record<string,unknown>={}){const onApply=vi.fn(),onClose=vi.fn(),adapter={inspectDocumentProperties:vi.fn().mockResolvedValue(source)};const props={adapter,sourceId:'s',canEdit:true,onApply,onClose,...extra};return {...render(<DocumentPropertiesDialog {...props}/>),props,onApply,onClose};}
it('applies an isolated draft and carries prior explicit edits',async()=>{
 const {onApply}=setup({metadata:{mode:'update',title:'Earlier edit'}});await screen.findByDisplayValue('Earlier edit');expect(screen.getByRole('button',{name:'Apply properties'})).toBeDisabled();fireEvent.change(screen.getByLabelText('Author'),{target:{value:'New author'}});expect(onApply).not.toHaveBeenCalled();fireEvent.click(screen.getByRole('button',{name:'Apply properties'}));expect(onApply).toHaveBeenCalledWith({mode:'update',title:'Earlier edit',author:'New author'});
});
it('explicit removal remains available for malformed XMP and reports its scope',async()=>{
 setup({adapter:{inspectDocumentProperties:vi.fn().mockResolvedValue({...source,error:'Malformed XMP'})}});await screen.findByDisplayValue('Original');expect(screen.getByLabelText('Title')).toBeDisabled();fireEvent.click(screen.getByLabelText('Remove document properties'));expect(screen.getByText(/page\/object metadata, attachments, comments and visible text/i)).toBeVisible();expect(screen.getByRole('button',{name:'Apply properties'})).toBeEnabled();
});
it('adding a public title after removal retains the cleared base',async()=>{
 const {onApply}=setup({metadata:{mode:'remove'}});await screen.findByLabelText('Title');await waitFor(()=>expect(screen.getByLabelText('Remove document properties')).toBeChecked());fireEvent.click(screen.getByLabelText('Remove document properties'));fireEvent.change(screen.getByLabelText('Title'),{target:{value:'Public title'}});fireEvent.click(screen.getByRole('button',{name:'Apply properties'}));expect(onApply).toHaveBeenCalledWith({mode:'update',clearExisting:true,title:'Public title'});
});
it('Cancel does not commit and Reset to source properties clears pending edits',async()=>{
 const first=setup();await screen.findByDisplayValue('Original');fireEvent.change(screen.getByLabelText('Title'),{target:{value:'Discard'}});fireEvent.click(screen.getByRole('button',{name:'Cancel'}));expect(first.onApply).not.toHaveBeenCalled();expect(first.onClose).toHaveBeenCalledOnce();first.unmount();
 const second=setup({metadata:{mode:'remove'}});await screen.findByLabelText('Title');fireEvent.click(screen.getByRole('button',{name:'Reset to source properties'}));fireEvent.click(screen.getByRole('button',{name:'Apply properties'}));expect(second.onApply).toHaveBeenCalledWith(undefined);
});
it('ignores an old source response after dialog ownership changes',async()=>{
 let old!:(p:DocumentProperties)=>void;const adapter={inspectDocumentProperties:vi.fn((id:string)=>id==='old'?new Promise<DocumentProperties>(r=>{old=r;}):Promise.resolve({...source,title:'Current source'}))};const props={adapter,sourceId:'old',canEdit:true,onApply:vi.fn(),onClose:vi.fn()};const view=render(<DocumentPropertiesDialog {...props}/>);await waitFor(()=>expect(old).toBeTypeOf('function'));view.rerender(<DocumentPropertiesDialog {...props} sourceId="new"/>);await screen.findByDisplayValue('Current source');await act(async()=>old(source));expect(screen.queryByDisplayValue('Original')).not.toBeInTheDocument();
});
it('read-only documents permit inspection but cannot apply removal or edits',async()=>{
 setup({canEdit:false});await screen.findByDisplayValue('Original');expect(screen.getByLabelText('Title')).toBeDisabled();expect(screen.getByLabelText('Remove document properties')).toBeDisabled();expect(screen.getByRole('button',{name:'Apply properties'})).toBeDisabled();
});
