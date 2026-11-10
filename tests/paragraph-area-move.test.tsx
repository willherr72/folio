import {cleanup,fireEvent,render} from '@testing-library/react';
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {ParagraphAreaMove} from '../src/components/ParagraphAreaMove';
import {moveParagraphBounds} from '../src/editor/paragraph-preview';
const bounds={x:40,y:50,width:100,height:80};
beforeEach(()=>{
 vi.stubGlobal('PointerEvent',class extends MouseEvent {pointerId:number;constructor(type:string,init:PointerEventInit={}){super(type,init);this.pointerId=init.pointerId??1;}});
 Object.defineProperty(SVGElement.prototype,'setPointerCapture',{configurable:true,value:vi.fn()});
 Object.defineProperty(SVGElement.prototype,'releasePointerCapture',{configurable:true,value:vi.fn()});
});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
function setup(){
 const props={bounds,pageWidth:400,pageHeight:500,onChange:vi.fn(),onDraggingChange:vi.fn()};
 const view=render(<svg><ParagraphAreaMove {...props}/></svg>),svg=view.container.querySelector('svg')!;
 Object.defineProperty(svg,'getScreenCTM',{value:()=>({inverse:()=>({})})});
 Object.defineProperty(svg,'createSVGPoint',{value:()=>({x:0,y:0,matrixTransform(){return{x:(this.x-10)/2,y:(this.y-20)/2};}})});
 const target=view.container.querySelector('.paragraph-area-move')!;
 return {...view,props,target,at:(x:number,y:number,pointerId=1)=>({clientX:10+x*2,clientY:20+y*2,pointerId,button:0})};
}
it('clamps the complete rectangle and refuses invalid geometry',()=>{
 expect(moveParagraphBounds(bounds,999,-20,400,500)).toEqual({x:300,y:0,width:100,height:80});
 expect(moveParagraphBounds(bounds,NaN,0,400,500)).toBeNull();
 expect(moveParagraphBounds({...bounds,width:500},0,0,400,500)).toBeNull();
});
it('previews a move locally at scaled coordinates and commits once on pointerup',()=>{
 const {props,target,at}=setup();fireEvent.pointerDown(target,at(50,60));
 expect(props.onDraggingChange).toHaveBeenLastCalledWith(true);
 fireEvent.pointerMove(target,at(100,100));expect(target).toHaveAttribute('x','90');expect(target).toHaveAttribute('y','90');
 expect(props.onChange).not.toHaveBeenCalled();fireEvent.pointerUp(target,at(100,100));
 expect(props.onChange).toHaveBeenCalledOnce();expect(props.onChange).toHaveBeenCalledWith({x:90,y:90,width:100,height:80});
 expect(props.onDraggingChange).toHaveBeenLastCalledWith(false);
});
it.each(['escape','cancel','lost capture','unmount','bounds change'])('discards a pending move on %s',reason=>{
 const view=setup(),{props,target,at}=view;fireEvent.pointerDown(target,at(50,60));fireEvent.pointerMove(target,at(100,100));
 if(reason==='escape')fireEvent.keyDown(window,{key:'Escape'});
 else if(reason==='cancel')fireEvent.pointerCancel(target,at(100,100));
 else if(reason==='lost capture')fireEvent.lostPointerCapture(target,at(100,100));
 else if(reason==='unmount')view.unmount();
 else view.rerender(<svg><ParagraphAreaMove {...props} bounds={{...bounds,x:60}}/></svg>);
 fireEvent.pointerUp(target,at(100,100));expect(props.onChange).not.toHaveBeenCalled();expect(props.onDraggingChange).toHaveBeenLastCalledWith(false);
});
it('ignores secondary pointers and preserves a no-op candidate',()=>{
 const {props,target,at}=setup();fireEvent.pointerDown(target,at(50,60));fireEvent.pointerDown(target,at(70,80,2));
 fireEvent.pointerMove(target,at(100,100,2));fireEvent.pointerUp(target,at(100,100,2));expect(props.onChange).not.toHaveBeenCalled();
 fireEvent.pointerUp(target,at(50,60));expect(props.onChange).not.toHaveBeenCalled();expect(props.onDraggingChange).toHaveBeenLastCalledWith(false);
});
it('preserves fractional original coordinates when a gesture does not move',()=>{
 const view=setup(),{props,target,at}=view;
 view.rerender(<svg><ParagraphAreaMove {...props} bounds={{...bounds,x:40.004,y:50.006}}/></svg>);
 fireEvent.pointerDown(target,at(50,60));fireEvent.pointerUp(target,at(50,60));
 expect(props.onChange).not.toHaveBeenCalled();
});
