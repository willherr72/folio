import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {WorkspaceReplacementDialog} from '../src/components/WorkspaceReplacementDialog';
import {nativeAdapter} from '../src/editor/adapter';
import {createSession} from '../src/editor/workspace';
import {captureWorkspaceReplacement} from '../src/editor/workspace-replacement';
import type {WorkspaceReplacementIntent} from '../src/editor/workspace-replacement-transport';
afterEach(cleanup);
async function setup(count:number){
 const adapter={...nativeAdapter,beginWorkspaceReplacement:vi.fn().mockResolvedValue('session'),cancelWorkspaceReplacement:vi.fn().mockResolvedValue(undefined),releaseWorkspaceReplacement:vi.fn().mockResolvedValue(undefined)};
 const session=createSession({id:'source',name:'Authored.pdf',pages:[{width:300,height:400}]},adapter,1),pageId=session.history.present.pages[0].id;
 adapter.planWorkspaceReplacement=vi.fn(async(_session:string,intent:WorkspaceReplacementIntent)=>{
  const matches=Array.from({length:count},(_,i)=>({matchIndex:i,overlayId:null,rects:[{x:30,y:40,width:30,height:12}],selected:intent.selected==null||intent.selected.some(s=>s.pageId===pageId&&s.matchIndex===i),blocker:null}));const selected=matches.filter(m=>m.selected).length;
  return {planId:'plan',query:intent.query,replacement:intent.replacement,options:intent.options,pages:[{pageId,matches}],selectedCount:selected,blockedCount:0,affectedPageIds:selected?[pageId]:[],batchBlocker:null,canApply:selected>0};
 });
 render(<WorkspaceReplacementDialog request={captureWorkspaceReplacement(session,session.sourceIds)} query="Alpha" options={{matchCase:false,wholeWords:false}} isCurrent={()=>true} borrowedIds={()=>session.sourceIds} onApply={()=>false} onCancel={vi.fn()}/>);
 fireEvent.change(screen.getByLabelText('Replace with'),{target:{value:'New'}});await waitFor(()=>expect(screen.getByRole('button',{name:'Check matches'})).toBeEnabled());fireEvent.click(screen.getByRole('button',{name:'Check matches'}));
 await waitFor(()=>expect(screen.getByRole('button',{name:'Preview replacements'})).toBeEnabled());return {adapter,pageId};
}
it('keeps matches visible while selecting an explicit subset and requires checking it again',async()=>{
 const {adapter,pageId}=await setup(3);fireEvent.click(screen.getByRole('checkbox',{name:/Page 1, match 1/}));
 expect(screen.getAllByRole('checkbox',{name:/Page 1, match/})).toHaveLength(3);expect(screen.getByRole('button',{name:'Preview replacements'})).toBeDisabled();
 fireEvent.click(screen.getByRole('checkbox',{name:/Page 1, match 2/}));fireEvent.click(screen.getByRole('button',{name:'Check matches'}));
 await waitFor(()=>expect(screen.getByRole('button',{name:'Preview replacements'})).toBeEnabled());
 expect(adapter.planWorkspaceReplacement).toHaveBeenLastCalledWith('session',expect.objectContaining({selected:[{pageId,matchIndex:2}]}));
});
it('bounds rendered matches and retains selection when navigating to another results page',async()=>{
 await setup(101);expect(screen.getAllByRole('checkbox',{name:/Page 1, match/})).toHaveLength(100);fireEvent.click(screen.getByRole('button',{name:'Next matches'}));
 expect(screen.getAllByRole('checkbox',{name:/Page 1, match/})).toHaveLength(1);expect(screen.getByRole('checkbox',{name:/Page 1, match 101/})).toBeChecked();
 fireEvent.click(screen.getByRole('button',{name:'Previous matches'}));expect(screen.getAllByRole('checkbox',{name:/Page 1, match/})).toHaveLength(100);
});
