import {useState} from "react";
import {createRoot} from "react-dom/client";
import {PageView} from "../../src/components/PageView";
import {ExistingTextDialog} from "../../src/components/ExistingTextDialog";
import {createDemoAdapter} from "../../src/editor/adapter";
import type {EditableTextRun,Rotation} from "../../src/editor/types";
import "../../src/styles.css";
const params=new URLSearchParams(location.search),intrinsic=Number(params.get("intrinsic")??0) as Rotation;
const rotation=Number(params.get("rotation")??0) as Rotation,zoom=Number(params.get("zoom")??100);
const width=intrinsic%180?320:240,height=intrinsic%180?240:320;
function rect(x:number,y:number,w:number,h:number){return intrinsic===90?{x:320-y-h,y:x,width:h,height:w}:intrinsic===180?{x:240-x-w,y:320-y-h,width:w,height:h}:intrinsic===270?{x:y,y:240-x-w,width:h,height:w}:{x,y,width:w,height:h};}
const sourceText=(source:string)=>source==="original"?"Original words":"New words";
const runs=(source:string):EditableTextRun[]=>[{objectIndex:0,text:sourceText(source),fontName:"Helvetica",fontSize:16,bounds:rect(30,40,140,20),supported:true}];
const transform={0:"",90:"translate(320 0) rotate(90)",180:"translate(240 320) rotate(180)",270:"translate(0 240) rotate(270)"}[intrinsic];
const adapter={...createDemoAdapter(),listTextRuns:async(source:string)=>({runs:runs(source)}),
 getPageText:async(source:string)=>({intrinsicRotation:intrinsic,characters:[...sourceText(source)].map((text,i)=>({text,...rect(30+i*8,40,8,20)}))}),
 renderPage:async(source:string)=>`data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}"><rect width="100%" height="100%" fill="white"/><g transform="${transform}"><text x="30" y="56" font-family="Arial" font-size="16">${sourceText(source)}</text></g></svg>`)}`};
function Fixture(){const[source,setSource]=useState("original"),[chosen,setChosen]=useState<EditableTextRun|null>(null),[tool,setTool]=useState<"edit"|"select">("edit");return <div style={{padding:30}}>
 {chosen&&<ExistingTextDialog run={chosen} busy={false} error={null} onCancel={()=>setChosen(null)} onApply={()=>{setSource("changed");setChosen(null);setTool("select");}}/>}
 <PageView adapter={adapter} page={{id:"page",sourceId:source,pageIndex:0,width,height,rotation,overlays:[]}} pageNumber={1} zoom={zoom} tool={tool} selectedOverlayId={null} pendingSignature={null} onSelectOverlay={()=>{}} onAddText={()=>{}} onPlaceSignature={()=>{}} onMoveOverlay={()=>{throw Error("Edit target started overlay drag");}} onEditText={setChosen}/>
 </div>;}
createRoot(document.getElementById("root")!).render(<Fixture/>);
