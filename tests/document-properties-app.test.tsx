import {cleanup,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {App} from '../src/App';
import {nativeAdapter} from '../src/editor/adapter';
import {pdfSaveApi} from '../src/editor/pdf-save';
import {splitPdfApi} from '../src/editor/pdf-split';
import {clearRenderCache} from '../src/components/PageView';
vi.mock('../src/editor/useWorkspaceRecovery',()=>({useWorkspaceRecovery:()=>({ready:true,checkpoint:vi.fn().mockResolvedValue(undefined)})}));
vi.mock('../src/components/DocumentViewport',()=>({DocumentViewport:()=>null}));
const source={title:'Original',author:'Author',subject:'Subject',keywords:'Keywords',creator:'Creator',producer:'Producer',creationDate:'2026',modificationDate:'2026',hasInfo:true,hasXmp:true,conflicts:[],multipleAuthors:false,error:null};
beforeEach(()=>{localStorage.clear();Object.defineProperty(window,'__TAURI_INTERNALS__',{value:{invoke:vi.fn().mockResolvedValue(0),transformCallback:()=>0},configurable:true});
 vi.spyOn(nativeAdapter,'openPdf').mockResolvedValue({id:'properties-source',name:'Properties.pdf',pages:[{width:300,height:400}]});vi.spyOn(nativeAdapter,'renderPage').mockResolvedValue('data:image/png;base64,AA');vi.spyOn(nativeAdapter,'closeDocument').mockResolvedValue();vi.spyOn(nativeAdapter as Required<typeof nativeAdapter>,'inspectDocumentProperties').mockResolvedValue(source);vi.spyOn(nativeAdapter,'exportPdf').mockResolvedValue('extracted.pdf');
 vi.spyOn(pdfSaveApi,'register').mockResolvedValue('job');vi.spyOn(pdfSaveApi,'cancel').mockResolvedValue(false);vi.spyOn(pdfSaveApi,'start').mockResolvedValue('saved.pdf');
 vi.spyOn(splitPdfApi,'register').mockResolvedValue('split');vi.spyOn(splitPdfApi,'cancel').mockResolvedValue(false);vi.spyOn(splitPdfApi,'start').mockResolvedValue({folder:'split-folder',count:1});
});
afterEach(()=>{cleanup();clearRenderCache(['properties-source']);vi.restoreAllMocks();delete(window as any).__TAURI_INTERNALS__;});
async function open(){render(<App initialDemo={false}/>);fireEvent.click(screen.getByRole('button',{name:'Open a PDF'}));await screen.findByRole('tab',{name:'Properties.pdf'});}
function menu(name:string){fireEvent.click(screen.getByRole('button',{name:'Save options'}));fireEvent.click(screen.getByRole('menuitem',{name}));}
async function edit(){menu('Document properties…');await screen.findByDisplayValue('Original');fireEvent.change(screen.getByLabelText('Title'),{target:{value:'Public title'}});fireEvent.click(screen.getByRole('button',{name:'Apply properties'}));await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());}
it('owns property drafts in document undo/redo and sends them to ordinary Save',async()=>{
 await open();await edit();expect(screen.getByLabelText('Unsaved changes')).toBeVisible();fireEvent.click(screen.getByRole('button',{name:'Undo'}));expect(screen.queryByLabelText('Unsaved changes')).toBeNull();fireEvent.click(screen.getByRole('button',{name:'Redo'}));menu('Document properties…');await screen.findByDisplayValue('Public title');fireEvent.change(screen.getByLabelText('Title'),{target:{value:'Cancelled'}});fireEvent.click(screen.getByRole('button',{name:'Cancel'}));
 fireEvent.click(screen.getByRole('button',{name:'Save a copy'}));await waitFor(()=>expect(pdfSaveApi.start).toHaveBeenCalledOnce());expect(vi.mocked(pdfSaveApi.start).mock.calls[0][1].metadata).toEqual({mode:'update',title:'Public title'});await waitFor(()=>expect(screen.queryByLabelText('Unsaved changes')).toBeNull());
});
it('carries property edits into extraction, split, and changed-security copies',async()=>{
 await open();await edit();menu('Extract pages…');fireEvent.click(screen.getByRole('button',{name:'Save extracted PDF…'}));await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());expect(vi.mocked(nativeAdapter.exportPdf).mock.calls[0][1]?.metadata).toEqual({mode:'update',title:'Public title'});
 menu('Split PDF…');fireEvent.click(screen.getByRole('button',{name:'Choose folder and split…'}));await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());expect(vi.mocked(splitPdfApi.start).mock.calls[0][1].metadata).toEqual({mode:'update',title:'Public title'});
 menu('Security settings…');fireEvent.change(screen.getByLabelText('Security'),{target:{value:'remove'}});fireEvent.click(screen.getByRole('button',{name:'Choose file and save…'}));await waitFor(()=>expect(screen.queryByRole('dialog')).toBeNull());expect(vi.mocked(pdfSaveApi.start).mock.calls[0][1]).toMatchObject({metadata:{mode:'update',title:'Public title'},security:{mode:'remove'}});expect(screen.getByLabelText('Unsaved changes')).toBeVisible();
});
