import {cleanup,render,screen,within} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {ReviewList} from '../src/components/ReviewList';
import type {NativeComment,PagePlan} from '../src/editor/types';
afterEach(cleanup);
it('keeps native and overlay entries in workspace page order, including duplicate pages',()=>{
 const first:PagePlan={id:'a',sourceId:'source',pageIndex:0,width:300,height:400,rotation:0,overlays:[{id:'overlay',type:'comment',x:20,y:20,text:'Overlay first',color:'#ffff00'}]};
 const second={...first,id:'b',pageIndex:1,overlays:[]};
 const note:NativeComment={pageIndex:1,annotationIndex:3,text:'Native second',bounds:{x:10,y:10,width:18,height:18}};
 render(<ReviewList pages={[first,second,{...second,id:'duplicate'}]} selectedPageId='a' selectedOverlayId={null} disabled={false} onSelect={vi.fn()} onSelectNative={vi.fn()} nativeNotes={new Map([['source',[note]]])}/>);
 expect(within(screen.getByRole('region',{name:'Document review'})).getAllByRole('button').map(b=>b.getAttribute('aria-label'))).toEqual(['Page 1: Overlay first','Page 2: Native second','Page 3: Native second']);
});
