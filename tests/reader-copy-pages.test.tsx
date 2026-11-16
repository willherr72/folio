import {cleanup,fireEvent,render,waitFor} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {PdfTextLayer,clearPageTextCache} from '../src/components/PdfTextLayer';
import {createDemoAdapter} from '../src/editor/adapter';
import type {PagePlan,PageText} from '../src/editor/types';

const source='copy-pages-source';
const text=(value:string):PageText=>({characters:Array.from(value,(text,index)=>({text,x:10+index*8,y:20,width:8,height:12}))});
const plan=(index:number):PagePlan=>({id:`copy-page-${index}`,sourceId:source,pageIndex:index,width:300,height:200,rotation:0,overlays:[]});
afterEach(()=>{cleanup();clearPageTextCache([source,'copy-pages-other']);window.getSelection()?.removeAllRanges();vi.restoreAllMocks();});

async function setup(values:string[],pages=values.map((_,index)=>plan(index)),other=false){
  const getPageText=vi.fn(async(id:string,index:number)=>text(id===source?values[index]:'Foreign'));
  const adapter={...createDemoAdapter(),getPageText};
  const view=render(<><div className="document-viewport" data-testid="copy-viewport"><svg>
    {pages.map((page,index)=><PdfTextLayer key={page.id} adapter={adapter} page={page} pageNumber={index+1} selectable/>)}</svg></div>
    {other&&<div className="document-viewport"><svg><PdfTextLayer adapter={adapter} page={{...plan(0),id:'other-copy-page',sourceId:'copy-pages-other'}} pageNumber={99} selectable/></svg></div>}</>);
  for(const [index,page]of pages.entries()){
    await waitFor(()=>expect(view.getByLabelText(`Page ${index+1} text`).textContent).toBe(page.protection?.permissions.copyContent===false?'':values[page.pageIndex]));
  }
  if(other)await waitFor(()=>expect(view.getByLabelText('Page 99 text').textContent).toBe('Foreign'));
  return {view,getPageText,layers:pages.map((_,index)=>view.getByLabelText(`Page ${index+1} text`))};
}
function nodes(layer:Element){return Array.from(layer.querySelectorAll('[data-pdf-character]'),span=>span.firstChild!);}
function copy(first:Node,last:Node,start=0,end=last.textContent!.length,backwards=false){
  const selection=window.getSelection()!;selection.removeAllRanges();
  if(backwards)selection.setBaseAndExtent(last,end,first,start);
  else{const range=document.createRange();range.setStart(first,start);range.setEnd(last,end);selection.addRange(range);}
  const setData=vi.fn();fireEvent.copy(document,{clipboardData:{setData}});
  expect(setData).toHaveBeenCalledOnce();return setData.mock.calls[0];
}
it('separates the last word of one page from the first word of the next',async()=>{
  const {layers}=await setup(['Hello','World']);
  expect(copy(nodes(layers[0])[0],nodes(layers[1]).at(-1)!)).toEqual(['text/plain','Hello\nWorld']);
});
it('keeps duplicated occurrences of one source page as distinct page boundaries',async()=>{
  const {layers}=await setup(['Same'],[plan(0),{...plan(0),id:'duplicate-copy-page'}]);
  expect(copy(nodes(layers[0])[0],nodes(layers[1]).at(-1)!)).toEqual(['text/plain','Same\nSame']);
});
it.each([['One\n','Two','One\nTwo'],['One\r\n','Two','One\r\nTwo'],['One\r','Two','One\rTwo'],['One','\nTwo','One\nTwo'],['One ',' Two','One \n Two']])('preserves source boundary whitespace for %j and %j',async(first,last,expected)=>{
  const {layers}=await setup([first,last]);
  expect(copy(nodes(layers[0])[0],nodes(layers[1]).at(-1)!)).toEqual(['text/plain',expected]);
});
it('does not invent an extra separator for an empty intervening page',async()=>{
  const {layers}=await setup(['First','','Last']);
  expect(copy(nodes(layers[0])[0],nodes(layers[2]).at(-1)!)).toEqual(['text/plain','First\nLast']);
});
it('does not copy crop-hidden text or count a hidden-only page as another boundary',async()=>{
  const pages=[plan(0),{...plan(1),crop:{x:150,y:100,width:30,height:30}},plan(2)];
  const {layers}=await setup(['First','Hidden','Last'],pages);
  expect(copy(nodes(layers[0])[0],nodes(layers[2]).at(-1)!)).toEqual(['text/plain','First\nLast']);
});
it('retains a complete supplementary scalar at a partial cross-page range boundary',async()=>{
  const {layers}=await setup(['Head𝑴','Tail']);
  expect(copy(nodes(layers[0]).at(-1)!,nodes(layers[1])[1],1,1)).toEqual(['text/plain','𝑴\nTa']);
});
it('copies backwards page selections in document order',async()=>{
  const {layers}=await setup(['First','Last']);
  expect(copy(nodes(layers[0])[0],nodes(layers[1]).at(-1)!,0,1,true)).toEqual(['text/plain','First\nLast']);
});
it('excludes restricted pages and text from another viewport',async()=>{
  const locked={...plan(1),protection:{documentId:'restricted-copy',revision:4,owner:false,permissions:{copyContent:false,editContent:false,annotate:true,fillForms:true,assemble:false,print:true,printHighQuality:false}}};
  const {view,layers,getPageText}=await setup(['First','Restricted','Last'],[plan(0),locked,plan(2)],true);
  expect(getPageText).not.toHaveBeenCalledWith(source,1);
  expect(copy(nodes(layers[0])[0],nodes(view.getByLabelText('Page 99 text')).at(-1)!)).toEqual(['text/plain','First\nLast']);
});
