import {useState} from "react";
import {createRoot} from "react-dom/client";
import {ExistingTextDialog} from "../../src/components/ExistingTextDialog";
import {fontApi} from "../../src/editor/custom-fonts";
import "../../src/styles.css";
const id="107244956e9962b9e96faccdc551825e0ae0898ae13737133e1b921a2fd35ffa";
const info={id,name:"DejaVu Serif",weight:400,italic:false,coverage:[[32,126],[233,233]] as [number,number][]};
fontApi.listInstalled=async()=>[{id:"installed",name:info.name,supported:true}];
fontApi.loadInstalled=async()=>info;fontApi.info=async()=>info;
fontApi.bytes=async()=>(await fetch("/tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).arrayBuffer();
fontApi.release=async()=>{};
const run={objectIndex:0,text:"Original sentence",fontName:"ABCDEF+DejaVuSerif",fontSize:14,bounds:{x:40,y:100,width:130,height:16},supported:true,isEmbedded:true,canSubstitute:true};
document.documentElement.dataset.theme=new URLSearchParams(location.search).get("theme")??"light";
function Fixture(){
 const[open,setOpen]=useState(false),[error,setError]=useState<string|null>(null),[result,setResult]=useState<unknown>(null);
 return <main style={{padding:40}}><button className="button" onClick={()=>setOpen(true)}>Edit embedded text</button><output aria-label="Applied replacement">{JSON.stringify(result)}</output>
 {open&&<ExistingTextDialog run={run} busy={false} error={error} onDraftChange={()=>setError(null)} onCancel={()=>setOpen(false)} onApply={(replacement,fontId)=>{
  if(!fontId){setError("The subset lacks é. Choose a substitute font.");return;}
  setResult({replacement,fontId});setOpen(false);
 }}/>}</main>;
}
createRoot(document.getElementById("root")!).render(<Fixture/>);
