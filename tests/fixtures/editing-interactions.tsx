// Isolated UI acceptance: real dialogs, synthetic text/rasters and no native file writes.
import {useState} from 'react';
import {createRoot} from 'react-dom/client';
import {TextParagraphDialog} from '../../src/components/TextParagraphDialog';
import {ImagePagesDialog} from '../../src/components/ImagePagesDialog';
import type {FolioAdapter} from '../../src/editor/adapter';
import type {EditableTextRun} from '../../src/editor/types';
import '../../src/styles.css';

const runs:EditableTextRun[]=Array.from({length:65},(_,i)=>({objectIndex:i,text:`Piece ${i}`,fontName:'Helvetica',fontSize:10,supported:true,bounds:{x:20+(i%5)*72,y:25+Math.floor(i/5)*25,width:60,height:12}}));
const svg='<svg xmlns="http://www.w3.org/2000/svg" width="400" height="380"><rect width="400" height="380" fill="white"/>'+runs.map(run=>`<text x="${run.bounds.x}" y="${run.bounds.y+10}" font-size="10">${run.text}</text>`).join('')+'</svg>';
const adapter:FolioAdapter={kind:'demo',openPdf:async()=>null,exportPdf:async()=>null,engineStatus:async()=>'Synthetic interaction fixture',closeDocument:async()=>{},renderPage:async()=>'data:image/svg+xml,'+encodeURIComponent(svg),listTextRuns:async()=>({runs}),inspectTextParagraph:async(_source,_page,indices)=>({objectIndices:indices,text:indices.map(index=>runs[index].text).join(' '),fontName:'Helvetica',fontSize:10,bounds:{x:20,y:25,width:350,height:325}})};
const proof={closed:[] as string[],previews:0};
(window as typeof window&{interactionProof:typeof proof}).interactionProof=proof;
adapter.closeDocument=async id=>{proof.closed.push(id);};
adapter.previewTextParagraph=async()=>({id:`candidate-${++proof.previews}`,name:'Synthetic candidate',pages:[{width:400,height:380}]});
function Fixture(){
 const [dialog,setDialog]=useState<'paragraph'|'images'|null>(null);
 return <main style={{padding:24}}><h1>Editing interaction fixture</h1><p>Synthetic data; no source files or native PDF operations.</p>
  <button className="button" onClick={()=>setDialog('paragraph')}>Paragraph fixture</button><button className="button" onClick={()=>setDialog('images')}>Image fixture</button>
  {dialog==='paragraph'&&<TextParagraphDialog adapter={adapter} sourceId="fixture" pageIndex={0} pageWidth={400} pageHeight={380} anchor={runs[0]} onApply={()=>false} onCancel={()=>setDialog(null)}/>}
  {dialog==='images'&&<ImagePagesDialog inserting={false} onPick={async()=>['C:\\fixtures\\scan.png']} onCreate={async()=>false} onClose={()=>setDialog(null)}/>}
 </main>;
}
createRoot(document.getElementById('root')!).render(<Fixture/>);
