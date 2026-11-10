import {useState} from "react";
import {createRoot} from "react-dom/client";
import {PageRaster, type PageRasterStatus} from "../../src/components/PageRaster";
import {pageTransform} from "../../src/editor/geometry";

type Settings = {url?:string;rotation:0|90|180|270;zoom:number;crop:boolean;inverse:boolean};
declare global {interface Window {rasterHarness:{show:(settings:Settings)=>void;status?:PageRasterStatus;clicks:number;present:(reference:boolean)=>void};}}
window.rasterHarness={show:()=>{},clicks:0,present(reference){
 document.querySelector<SVGGElement>("#candidate")!.style.display=reference?"none":"";
 document.querySelector<SVGGElement>("#reference")!.style.display=reference?"":"none";
}};
function Fixture(){
 const [settings,setSettings]=useState<Settings>({rotation:0,zoom:1,crop:false,inverse:false});
 window.rasterHarness.show=setSettings;
 const {url,rotation,zoom,crop,inverse}=settings;
 const width=200,height=260,turned=rotation===90||rotation===270;
 const fullWidth=turned?height:width,fullHeight=turned?width:height;
 const dimensions=crop?{x:13,y:17,width:fullWidth-31,height:fullHeight-43}:{x:0,y:0,width:fullWidth,height:fullHeight};
 const rasterWidth=inverse?height:width,rasterHeight=inverse?width:height;
 const transform=inverse?pageTransform(height,width,270):undefined;
 const rasterProps={url,width:rasterWidth,height:rasterHeight,transform};
 return <><input aria-label="Draft" defaultValue="Keep focus"/><button id="after">After</button>
  <svg id="page" width={dimensions.width*zoom} height={dimensions.height*zoom} viewBox={`${dimensions.x} ${dimensions.y} ${dimensions.width} ${dimensions.height}`} style={{display:"block",background:"#dbe6ef"}} onPointerDown={()=>window.rasterHarness.clicks++}>
   <g transform={pageTransform(width,height,rotation)||undefined}>
    <g id="candidate"><PageRaster {...rasterProps} onStatus={status=>{window.rasterHarness.status=status;}}/></g>
    <g id="reference" style={{display:"none"}}><image {...rasterProps} href={url} preserveAspectRatio="none"/></g>
    <rect id="overlay" x={40} y={40} width={25} height={25} fill="#f30071" tabIndex={0} aria-label="Overlay"/>
   </g>
  </svg></>;
}
createRoot(document.getElementById("root")!).render(<Fixture/>);
