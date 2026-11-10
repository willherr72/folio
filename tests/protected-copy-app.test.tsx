import {act,cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {App} from '../src/App';
import {nativeAdapter} from '../src/editor/adapter';
import {pdfSaveApi} from '../src/editor/pdf-save';
import {clearRenderCache} from '../src/components/PageView';
vi.mock('../src/editor/useWorkspaceRecovery',()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock('../src/components/DocumentViewport',()=>({DocumentViewport:()=>null}));
let finish:(path:string|null)=>void;
beforeEach(()=>{localStorage.clear();Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});vi.spyOn(nativeAdapter,'openPdf').mockResolvedValue({id:'secure-source',name:'Security.pdf',pages:[{width:300,height:400}]});vi.spyOn(nativeAdapter,'renderPage').mockResolvedValue('data:image/png;base64,AA');vi.spyOn(nativeAdapter,'closeDocument').mockResolvedValue();vi.spyOn(pdfSaveApi,'register').mockResolvedValue('job');vi.spyOn(pdfSaveApi,'cancel').mockResolvedValue(true);vi.spyOn(pdfSaveApi,'start').mockImplementation(()=>new Promise(done=>{finish=done;}));});
afterEach(()=>{cleanup();clearRenderCache(['secure-source']);vi.restoreAllMocks();delete(window as any).__TAURI_INTERNALS__;});
it('exports a changed-security copy without marking the active tab saved or changing undo',async()=>{
 render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole('button',{name:'Open a PDF'}));await screen.findByRole('tab',{name:'Security.pdf'});fireEvent.click(screen.getByRole('button',{name:'Rotate'}));fireEvent.click(screen.getByRole('button',{name:'Save options'}));fireEvent.click(screen.getByRole('menuitem',{name:'Security settings…'}));await screen.findByRole('dialog',{name:'Security settings'});
 fireEvent.change(screen.getByLabelText('Security'),{target:{value:'remove'}});fireEvent.click(screen.getByRole('button',{name:'Choose file and save…'}));await waitFor(()=>expect(pdfSaveApi.start).toHaveBeenCalledOnce());expect(vi.mocked(pdfSaveApi.start).mock.calls[0][1]).toMatchObject({pages:[{rotation:90}],security:{mode:'remove'}});expect(screen.getByLabelText('Unsaved changes')).toBeInTheDocument();fireEvent.keyDown(window,{key:'s',ctrlKey:true});expect(pdfSaveApi.start).toHaveBeenCalledOnce();
 await act(async()=>finish('security-copy.pdf'));await screen.findByText('Saved a copy with new security settings to security-copy.pdf');expect(screen.queryByRole('dialog')).toBeNull();expect(screen.getByLabelText('Unsaved changes')).toBeInTheDocument();expect(screen.getByRole('button',{name:'Undo'})).toBeEnabled();fireEvent.click(screen.getByRole('button',{name:'Undo'}));expect(screen.queryByLabelText('Unsaved changes')).toBeNull();
 expect(JSON.stringify(localStorage)).not.toContain('ownerPassword');
});
