import {useState} from "react";
import {createRoot} from "react-dom/client";
import {StyledParagraphDialog} from "../../src/components/StyledParagraphDialog";
import {styledEditorFixture} from "./styled-editor-data";
import type {FolioAdapter} from "../../src/editor/adapter";
import type {StyledParagraphLayoutRequest} from "../../src/editor/types";
import "../../src/styles.css";
const requests:StyledParagraphLayoutRequest[]=[],closed:string[]=[];
const source=styledEditorFixture();
const raster="data:image/svg+xml,"+encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="400" height="500"><rect width="400" height="500" fill="white"/><text x="20" y="42" font-size="12">one</text><text x="48" y="42" font-size="20" fill="#882244">two</text></svg>');
const runs=source.runs.map((run,objectIndex)=>({objectIndex,text:run.text,fontName:source.styles[objectIndex].fontName,fontSize:source.styles[objectIndex].fontSize,bounds:{x:20+objectIndex*30,y:30,width:30,height:20},supported:true}));
const unused=async():Promise<never>=>{throw new Error("Not used by this dialog fixture.");};
const adapter:FolioAdapter={kind:"native",openPdf:unused,exportPdf:unused,engineStatus:unused,listTextRuns:async()=>({runs}),suggestStyledParagraph:async()=>source,inspectStyledParagraph:async()=>source,previewStyledParagraph:async(_source:string,_page:number,request:StyledParagraphLayoutRequest)=>{requests.push(request);return {id:`preview-${requests.length}`,name:"preview.pdf",pages:[{width:400,height:500}]};},renderPage:async()=>raster,closeDocument:async(id:string)=>{closed.push(id);}};
declare global {interface Window {styledDialogAcceptance:{requests:StyledParagraphLayoutRequest[];closed:string[]}}}
window.styledDialogAcceptance={requests,closed};
function Fixture(){const [open,setOpen]=useState(true),[applied,setApplied]=useState("");return <><main><h1>Paragraph dialog acceptance</h1><button onClick={()=>setOpen(true)}>Open paragraph</button><p role="status">{applied}</p></main>{open&&<StyledParagraphDialog adapter={adapter} sourceId="source" pageIndex={0} pageWidth={400} pageHeight={500} anchor={runs[0]} onApply={doc=>{setApplied(doc.id);setOpen(false);return true;}} onCancel={()=>setOpen(false)}/>}</>;}
createRoot(document.getElementById("root")!).render(<Fixture/>);
