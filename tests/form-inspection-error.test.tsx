import {cleanup,render,screen} from '@testing-library/react';
import {afterEach,expect,it,vi} from 'vitest';
import {FormFillEditor} from '../src/components/FormFillEditor';
import {nativeAdapter} from '../src/editor/adapter';
import type {PagePlan} from '../src/editor/types';
afterEach(cleanup);
const page:PagePlan={id:'p',sourceId:'s',pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
it('shows inspection refusal without claiming the page is empty, then recovers on another page',async()=>{
 const adapter={...nativeAdapter,listFormFields:vi.fn().mockRejectedValueOnce(new Error('Field inspection exceeds its memory budget')).mockResolvedValueOnce([])};
 const props={page,adapter,pageNumber:1,preview:false,onApply:vi.fn(),onClose:vi.fn()};
 const view=render(<FormFillEditor {...props}/>);
 expect(await screen.findByRole('alert')).toHaveTextContent('memory budget');
 expect(screen.queryByText(/No interactive form fields/)).not.toBeInTheDocument();
 expect(screen.getByRole('button',{name:'Done'})).toBeEnabled();
 view.rerender(<FormFillEditor {...props} page={{...page,id:'p2',pageIndex:1}} pageNumber={2}/>);
 expect(await screen.findByText(/No interactive form fields/)).toBeInTheDocument();
 expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});
