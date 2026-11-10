import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import type {ComponentProps} from 'react';
import type {DocumentViewport} from '../src/components/DocumentViewport';
import {App} from '../src/App';
import {createDemoAdapter} from '../src/editor/adapter';
import {createSession,type Workspace} from '../src/editor/workspace';
import {commit} from '../src/editor/model';

const seed=vi.hoisted(()=>({workspace:null as Workspace|null}));
vi.mock('../src/editor/workspace',async original=>({...await original<typeof import('../src/editor/workspace')>(),emptyWorkspace:()=>seed.workspace}));
vi.mock('../src/components/DocumentViewport',()=>({DocumentViewport:(p:ComponentProps<typeof DocumentViewport>)=>{
 const page=p.pages[0],overlay=page.overlays[0];
 return <div><output data-testid="geometry">{JSON.stringify(overlay)}</output>{(['start','move','end'] as const).map(phase=><button key={phase} onClick={()=>p.onMoveOverlay(page.id,overlay.id,phase==='move'?25:0,0,phase)}>Gesture {phase}</button>)}</div>;
}}));
beforeEach(()=>{
 localStorage.clear();const adapter=createDemoAdapter();const a=createSession({id:'a',name:'A.pdf',pages:[{width:600,height:800}]},adapter,100);
 a.history.present={...a.history.present,pages:a.history.present.pages.map(page=>({...page,overlays:[{id:'note',type:'comment',x:10,y:20,text:'Note',color:'#ffff00'}]}))};
 for(let i=1;i<=100;i++)a.history=commit(a.history,doc=>({...doc,pages:doc.pages.map(page=>({...page,overlays:page.overlays.map(overlay=>({...overlay,x:10+i}))}))}));
 const b=createSession({id:'b',name:'B.pdf',pages:[{width:600,height:800}]},adapter,100);
 seed.workspace={tabs:[a,b],activeId:a.id};
});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
const button=(name:string)=>screen.getByRole('button',{name});
const x=()=>JSON.parse(screen.getByTestId('geometry').textContent!).x;
it.each(['end','tab-switch'])('bounds completed gesture history through %s without losing the drag start',mode=>{
 render(<App initialDemo={false}/>);expect(x()).toBe(110);
 fireEvent.click(button('Gesture start'));fireEvent.click(button('Gesture move'));expect(x()).toBe(135);
 if(mode==='end')fireEvent.click(button('Gesture end'));
 else{fireEvent.click(screen.getByRole('tab',{name:'B.pdf'}));fireEvent.click(screen.getByRole('tab',{name:'A.pdf'}));}
 fireEvent.click(button('Undo'));expect(x()).toBe(110);fireEvent.click(button('Redo'));expect(x()).toBe(135);
 for(let i=0;i<100;i++)fireEvent.click(button('Undo'));
 expect(x()).toBe(11);expect(button('Undo')).toBeDisabled();
 for(let i=0;i<100;i++)fireEvent.click(button('Redo'));
 expect(x()).toBe(135);expect(button('Redo')).toBeDisabled();
});
it('does not consume a history slot or clear Redo for an unchanged drag',()=>{
 render(<App initialDemo={false}/>);fireEvent.click(button('Undo'));expect(x()).toBe(109);
 fireEvent.click(button('Gesture start'));fireEvent.click(button('Gesture end'));
 expect(button('Redo')).toBeEnabled();fireEvent.click(button('Redo'));expect(x()).toBe(110);
 for(let i=0;i<100;i++)fireEvent.click(button('Undo'));
 expect(x()).toBe(10);expect(button('Undo')).toBeDisabled();
});
