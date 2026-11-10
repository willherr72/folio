import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {PdfSaveProgressDialog,type PdfSaveView} from '../src/components/PdfSaveProgressDialog';
afterEach(cleanup);
const view:PdfSaveView={progress:null,cancelling:false,finishing:false,error:null};
it('keeps focus in the modal, treats Escape as cancellation and restores the opener',async()=>{
 const opener=document.createElement('button');document.body.append(opener);opener.focus();const cancel=vi.fn();
 const rendered=render(<PdfSaveProgressDialog view={view} onCancel={cancel}/>);const button=screen.getByRole('button',{name:'Cancel save'});expect(button).toHaveFocus();
 fireEvent.keyDown(button,{key:'Escape'});expect(cancel).toHaveBeenCalledOnce();expect(screen.getByRole('dialog')).toBeInTheDocument();
 rendered.rerender(<PdfSaveProgressDialog view={{...view,cancelling:true}} onCancel={cancel}/>);fireEvent.keyDown(button,{key:'Tab'});expect(screen.getByRole('dialog')).toHaveFocus();fireEvent.keyDown(screen.getByRole('dialog'),{key:'Escape'});expect(cancel).toHaveBeenCalledOnce();
 rendered.unmount();await Promise.resolve();expect(opener).toHaveFocus();opener.remove();
});
it('shows indeterminate progress outside page stages and disables cancellation during final publication',()=>{
 const cancel=vi.fn();const rendered=render(<PdfSaveProgressDialog view={{...view,progress:{jobId:'j',stage:'preserving',completed:0,total:42}}} onCancel={cancel}/>);
 expect(screen.getByRole('progressbar')).not.toHaveAttribute('value');
 rendered.rerender(<PdfSaveProgressDialog view={{...view,finishing:true}} onCancel={cancel}/>);expect(screen.getByRole('button',{name:'Cancel save'})).toBeDisabled();fireEvent.keyDown(screen.getByRole('dialog'),{key:'Escape'});expect(cancel).not.toHaveBeenCalled();
});
