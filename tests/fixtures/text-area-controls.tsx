// Real App and native text preparation; file dialogs/export are captured for browser assertions.
import { createRoot } from "react-dom/client";
import { App } from "../../src/App";
import { nativeAdapter } from "../../src/editor/adapter";
import { fontApi, type FontInfo } from "../../src/editor/custom-fonts";
import { shapedTextApi } from "../../src/editor/shaped-text";
import type { PagePlan } from "../../src/editor/types";

const bridge="http://127.0.0.1:1423";
const info:FontInfo=await (await fetch(`${bridge}/info`)).json();
fontApi.listInstalled=async()=>[{id:"fixture",name:info.name,supported:true}];
fontApi.loadInstalled=async()=>info;fontApi.info=async()=>info;fontApi.release=async()=>{};
fontApi.bytes=async()=>await (await fetch("/tests/fixtures/shaped-text/DejaVuSans.ttf")).arrayBuffer();
shapedTextApi.prepare=async overlay=>{
  const response=await (await fetch(`${bridge}/prepare`,{method:"POST",body:JSON.stringify(overlay)})).json();
  if(response.error)throw new Error(response.error);
  return response.result;
};
nativeAdapter.openPdf=async()=>({id:"area-ui",name:"Text area workspace.pdf",pages:[{width:500,height:640}]});
nativeAdapter.renderPage=async()=>"data:image/svg+xml,"+encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="500" height="640"><rect width="500" height="640" fill="white"/></svg>');
nativeAdapter.getPageText=async()=>({characters:[]});nativeAdapter.closeDocument=async()=>{};nativeAdapter.engineStatus=async()=>"Native area fixture";
nativeAdapter.exportPdf=async(pages,options)=>{
  (window as typeof window & {savedAreaPlan?:{pages:PagePlan[];flatten:boolean}}).savedAreaPlan={pages,flatten:options?.flatten??false};
  return "Area fixture plan captured";
};
createRoot(document.getElementById("root")!).render(<App initialDemo={false}/>);
