import {useEffect,useRef} from 'react';
import type {TextRemovalWorkflow} from '../editor/text-removal-workflow';
import type {TextRemovalPreview} from '../editor/text-removal';
interface Props {workflow:TextRemovalWorkflow|null;preview:TextRemovalPreview|null;pageIndex:number;width:number;height:number;onReady(ready:boolean,error?:string):void}
export function TextRemovalGlyphMask({workflow,preview,pageIndex,width,height,onReady}:Props){
 const canvas=useRef<HTMLCanvasElement>(null),notify=useRef(onReady);notify.current=onReady;
 useEffect(()=>{
  let alive=true;notify.current(false);
  const node=canvas.current;if(!node)return;
  const scale=Math.min(900/width,4096/height);node.width=Math.max(1,Math.ceil(width*scale));node.height=Math.max(1,Math.ceil(height*scale));
  const context=node.getContext('2d');context?.clearRect(0,0,node.width,node.height);
  if(!workflow||!preview)return;
  const page=preview.pages.find(p=>p.pageIndex===pageIndex);
  if(!page){notify.current(true);return;}
  if(!context){notify.current(false,'The selection preview could not be drawn.');return;}
  context.fillStyle='rgba(190, 35, 45, 0.45)';
  void(async()=>{
   for(let offset=0;offset<page.selectedGlyphCount;){
    const chunk=await workflow.bounds(preview,pageIndex,offset);if(!alive)return;
    for(const box of chunk.boxes)context.fillRect(box.x*scale,box.y*scale,box.width*scale,box.height*scale);
    offset+=chunk.boxes.length;
   }
   if(alive)notify.current(true);
  })().catch(error=>{if(alive)notify.current(false,error instanceof Error?error.message:String(error));});
  return()=>{alive=false;};
 },[workflow,preview,pageIndex,width,height]);
 return <canvas ref={canvas} className="text-removal-glyphs" aria-hidden="true"/>;
}
