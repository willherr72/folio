// Real application UI with synthetic native services; no user files or native writes.
import {useEffect} from "react";
import {createRoot} from "react-dom/client";
import {App} from "../../src/App";
import {nativeAdapter} from "../../src/editor/adapter";
import {DEFAULT_PREFERENCES,PREFERENCES_KEY} from "../../src/editor/preferences";
import type {DocumentInfo} from "../../src/editor/types";
import "../../src/styles.css";
const params=new URLSearchParams(location.search),reader=params.has("reader");
localStorage.setItem(PREFERENCES_KEY,JSON.stringify({...DEFAULT_PREFERENCES,theme:params.has("dark")?"dark":"light"}));
const info:DocumentInfo={id:"toolbar-fixture",name:reader?"Protected fixture.pdf":"Toolbar fixture.pdf",pages:[{width:612,height:792}],...(reader?{protection:{documentId:"reader-fixture",owner:false,revision:6,permissions:{print:true,printHighQuality:true,copyContent:true,editContent:false,annotate:false,fillForms:true,assemble:false}}}:{})};
const svg='<svg xmlns="http://www.w3.org/2000/svg" width="612" height="792"><rect width="612" height="792" fill="white"/><text x="50" y="70" font-family="sans-serif" font-size="20">Toolbar layout fixture</text><text x="50" y="105" font-family="sans-serif" font-size="12">Synthetic page. No source files or native writes.</text></svg>';
nativeAdapter.openPdf=async()=>info;
nativeAdapter.renderPage=async()=>"data:image/svg+xml,"+encodeURIComponent(svg);
nativeAdapter.engineStatus=async()=>"Synthetic toolbar acceptance service";
nativeAdapter.closeDocument=async()=>{};
nativeAdapter.getPageText=async()=>({characters:[]});
nativeAdapter.listTextRuns=async()=>({runs:[]});
nativeAdapter.listOccurrenceFormTextRuns=undefined;nativeAdapter.listFormTextRuns=async()=>({runs:[]});
nativeAdapter.exportPdf=async()=>"Synthetic saved copy.pdf";
function Fixture(){useEffect(()=>{(document.querySelector('.empty-actions button') as HTMLButtonElement|null)?.click();},[]);return <App initialDemo={false}/>;}
createRoot(document.getElementById("root")!).render(<Fixture/>);
