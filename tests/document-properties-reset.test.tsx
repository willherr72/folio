import {it,expect,vi,afterEach} from 'vitest';
import {render,screen,fireEvent,cleanup} from '@testing-library/react';
import {DocumentPropertiesDialog} from '../src/components/DocumentPropertiesDialog';
afterEach(cleanup);
it('resetting a pending removal restores the unchanged source even when its XMP is malformed',async()=>{
 const source={title:'Original',author:'Author',subject:'',keywords:'',creator:'',producer:'',creationDate:'',modificationDate:'',hasInfo:true,hasXmp:true,conflicts:[],multipleAuthors:false,error:'Malformed XMP'};
 const onApply=vi.fn();render(<DocumentPropertiesDialog adapter={{inspectDocumentProperties:vi.fn().mockResolvedValue(source)}} sourceId="s" metadata={{mode:'remove'}} canEdit onApply={onApply} onClose={vi.fn()}/>);
 await screen.findByLabelText('Title');fireEvent.click(screen.getByRole('button',{name:'Reset to source properties'}));expect(screen.getByRole('button',{name:'Apply properties'})).toBeEnabled();fireEvent.click(screen.getByRole('button',{name:'Apply properties'}));expect(onApply).toHaveBeenCalledWith(undefined);
});
