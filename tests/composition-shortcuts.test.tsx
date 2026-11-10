import {cleanup, createEvent, fireEvent, render, screen, waitFor} from '@testing-library/react';
import {useState} from 'react';
import {afterEach, expect, it, vi} from 'vitest';
import {Modal} from '../src/components/Modal';
import {DocumentPropertiesDialog} from '../src/components/DocumentPropertiesDialog';
import {SearchBar} from '../src/components/SearchBar';
import type {DocumentProperties} from '../src/editor/document-properties';

afterEach(cleanup);
const compositionModes = [{isComposing: true}, {isComposing: false, keyCode: 229}];
const properties: DocumentProperties = {title:'Original',author:'',subject:'',keywords:'',creator:'',producer:'',creationDate:'',modificationDate:'',hasInfo:true,hasXmp:false,conflicts:[],multipleAuthors:false,error:null};
const adapter = {inspectDocumentProperties: async () => properties};

it.each(compositionModes)('keeps a properties draft through composition Escape (%j)', async mode => {
  const apply = vi.fn(), backgroundKey = vi.fn();
  function Example() {
    const [open, setOpen] = useState(false);
    return <div onKeyDown={backgroundKey}><button onClick={()=>setOpen(true)}>Properties</button>{open && <DocumentPropertiesDialog adapter={adapter} sourceId="source" canEdit onApply={apply} onClose={()=>setOpen(false)}/>}</div>;
  }
  render(<Example/>);
  const trigger = screen.getByText('Properties');
  trigger.focus(); fireEvent.click(trigger);
  const title = await screen.findByLabelText('Title');
  title.focus(); fireEvent.change(title, {target: {value: 'Edited 日本語'}});
  const escape = createEvent.keyDown(title, {key:'Escape', ...mode});
  fireEvent(title, escape);
  expect(screen.getByRole('dialog')).toBeInTheDocument();
  expect(title).toHaveValue('Edited 日本語');
  expect(title).toHaveFocus();
  expect(escape.defaultPrevented).toBe(false);
  expect(backgroundKey).not.toHaveBeenCalled();
  expect(apply).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText('Apply properties'));
  expect(apply).toHaveBeenCalledOnce();
  expect(apply).toHaveBeenCalledWith({mode:'update', title:'Edited 日本語'});
  fireEvent.keyDown(screen.getByText('Cancel'), {key:'Escape'});
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  await waitFor(()=>expect(trigger).toHaveFocus());
});

it.each(compositionModes)('leaves composition Tab to the input method (%j)', mode => {
  render(<Modal title="Edit" onClose={()=>{}}><input aria-label="Draft"/><button>Last action</button></Modal>);
  const input = screen.getByLabelText('Draft'), last = screen.getByText('Last action');
  input.focus();
  const composingTab = createEvent.keyDown(input, {key:'Tab', shiftKey:true, ...mode});
  fireEvent(input, composingTab);
  expect(composingTab.defaultPrevented).toBe(false);
  expect(input).toHaveFocus();
  fireEvent.keyDown(input, {key:'Tab', shiftKey:true});
  expect(last).toHaveFocus();
  fireEvent.keyDown(last, {key:'Tab'});
  expect(input).toHaveFocus();
});

it.each(compositionModes)('keeps search open and its match position through composition keys (%j)', mode => {
  const backgroundKey = vi.fn();
  function Example() {
    const [open,setOpen] = useState(true), [query,setQuery] = useState('日本'), [index,setIndex] = useState(1);
    return <div onKeyDown={backgroundKey}>{open ? <SearchBar query={query} onQueryChange={setQuery} index={index} total={3} searching={false} error={null} hasText onNext={()=>setIndex(i=>i+1)} onPrevious={()=>setIndex(i=>i-1)} onClose={()=>setOpen(false)}/> : <p>Search closed</p>}</div>;
  }
  render(<Example/>);
  const input = screen.getByRole('searchbox');
  for (const key of [{key:'Enter'}, {key:'Enter',shiftKey:true}, {key:'Escape'}]) {
    const event = createEvent.keyDown(input, {...key,...mode});
    fireEvent(input, event);
    expect(screen.getByRole('searchbox')).toHaveValue('日本');
    expect(screen.getByRole('status')).toHaveTextContent('2 of 3');
    expect(event.defaultPrevented).toBe(false);
  }
  expect(backgroundKey).not.toHaveBeenCalled();
  fireEvent.keyDown(input, {key:'Enter'});
  expect(screen.getByRole('status')).toHaveTextContent('3 of 3');
  fireEvent.keyDown(input, {key:'Enter',shiftKey:true});
  expect(screen.getByRole('status')).toHaveTextContent('2 of 3');
  fireEvent.keyDown(input, {key:'Escape'});
  expect(screen.queryByRole('searchbox')).not.toBeInTheDocument();
  expect(screen.getByText('Search closed')).toBeVisible();
});
