import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {SearchBar,type SearchBarProps} from '../src/components/SearchBar';
afterEach(cleanup);

function props():SearchBarProps{return {query:'Alpha',onQueryChange:vi.fn(),index:0,total:1,searching:false,error:null,hasText:true,onNext:vi.fn(),onPrevious:vi.fn(),onClose:vi.fn()};}
it('opens Replace from an accessible search action without changing the query or selection',()=>{
 const base=props(),onReplace=vi.fn();render(<SearchBar {...({...base,onReplace} as SearchBarProps)}/>);
 fireEvent.click(screen.getByRole('button',{name:'Replace text'}));
 expect(onReplace).toHaveBeenCalledTimes(1);expect(base.onQueryChange).not.toHaveBeenCalled();expect(base.onNext).not.toHaveBeenCalled();expect(base.onClose).not.toHaveBeenCalled();
});
it('handles Ctrl+H inside Find and keeps composition and Alt combinations from opening Replace',()=>{
 const base=props(),onReplace=vi.fn();render(<SearchBar {...({...base,onReplace} as SearchBarProps)}/>);
 const input=screen.getByRole('searchbox',{name:'Find in document'});
 expect(fireEvent.keyDown(input,{key:'h',ctrlKey:true})).toBe(false);expect(onReplace).toHaveBeenCalledTimes(1);
 fireEvent.keyDown(input,{key:'h',ctrlKey:true,altKey:true});fireEvent.keyDown(input,{key:'h',ctrlKey:true,isComposing:true});
 expect(onReplace).toHaveBeenCalledTimes(1);
});
it('keeps the entry available with an empty query and pending Find results',()=>{
 const onReplace=vi.fn();render(<SearchBar {...({...props(),query:'',searching:true,onReplace} as SearchBarProps)}/>);
 expect(screen.getByRole('button',{name:'Replace text'})).toBeEnabled();
 fireEvent.click(screen.getByRole('button',{name:'Replace text'}));expect(onReplace).toHaveBeenCalledTimes(1);
});
it('omits the action when the document has no replacement service',()=>{
 render(<SearchBar {...props()}/>);expect(screen.queryByRole('button',{name:'Replace text'})).not.toBeInTheDocument();
});
