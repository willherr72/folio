import {pageVisibleRect,displayDimensions} from "../editor/geometry";
import {useEffect,useMemo,useState,type MouseEvent} from 'react';
import {Modal} from './Modal';
import type {FolioAdapter} from '../editor/adapter';
import type {EditorDocument,SourceReplacement} from '../editor/types';
import {selectPages} from '../editor/page-selection';
import {useOcrWorkflow} from '../editor/ocr/useOcrWorkflow';
import {ocrTargets,ocrWords,editOcrWord,removeOcrWord,type OcrTarget} from '../editor/ocr/workflow';
import type {OcrLanguage,OcrBounds} from '../editor/ocr/types';
import './ocr.css';

const languages:[OcrLanguage,string][]=[['eng','English'],['fra','French'],['spa','Spanish']];
const rectangle=(b:OcrBounds)=>`M${b.x} ${b.y}h${b.width}v${b.height}h${-b.width}Z`;
export function OcrDialog({document,adapter,knownSourceIds,onAdopt,onClose}:{document:EditorDocument;adapter:FolioAdapter;
 knownSourceIds():ReadonlySet<string>;onAdopt(replacements:SourceReplacement[]):boolean;onClose():void}){
 const workflow=useOcrWorkflow({adapter,knownSourceIds,onAdopt});
 const [mode,setMode]=useState<'current'|'all'|'range'>('current'),[range,setRange]=useState(''),[chosen,setChosen]=useState<OcrLanguage[]>(['eng']);
 const [selected,setSelected]=useState(0),[value,setValue]=useState(''),[uncertainOnly,setUncertainOnly]=useState(false),[listPage,setListPage]=useState(0);
 const draft=workflow.drafts[workflow.active],words=useMemo(()=>draft?ocrWords(draft.page):[],[draft]);
 const cropPage=draft?.target.page;
 const cropBox=cropPage?.crop&&draft?(()=>{const full=displayDimensions(cropPage.width,cropPage.height,cropPage.rotation),rect=pageVisibleRect(cropPage);return {x:rect.x/full.width*draft.page.width,y:rect.y/full.height*draft.page.height,width:rect.width/full.width*draft.page.width,height:rect.height/full.height*draft.page.height};})():null;
 const excluded=useMemo(()=>new Set(draft?.excluded??[]),[draft]),removed=useMemo(()=>new Set(draft?.removed??[]),[draft]);
 const word=words[selected];
 const busy=workflow.stage==='recognizing'||workflow.stage==='applying'||workflow.previewLoading,editable=!!word&&!excluded.has(selected)&&!removed.has(selected);
 const wordDirty=editable&&value!==word.text;
 const uncertain=useMemo(()=>words.flatMap((word,i)=>word.confidence<80&&!excluded.has(i)&&!removed.has(i)?[i]:[]),[words,excluded,removed]);
 const uncertainSet=useMemo(()=>new Set(uncertain),[uncertain]);
 const visible=useMemo(()=>words.flatMap((word,i)=>!uncertainOnly||uncertainSet.has(i)?[{word,index:i}]:[]),[words,uncertainOnly,uncertainSet]);
 const listPages=Math.max(1,Math.ceil(visible.length/50)),offset=Math.min(listPage,listPages-1)*50;
 useEffect(()=>{setSelected(uncertain[0]??words.findIndex((_,i)=>!excluded.has(i)));setListPage(0);},[workflow.active,workflow.drafts.length]);
 useEffect(()=>{setValue(word?.text??'');},[selected,word?.text]);
 const paths=useMemo(()=>{
  const result={existing:'',uncertain:'',accepted:'',removed:''};
  words.forEach((word,i)=>{const kind=excluded.has(i)?'existing':removed.has(i)?'removed':word.confidence<80?'uncertain':'accepted';result[kind]+=rectangle(word.bounds);});return result;
 },[words,excluded,removed]);
 let targets:OcrTarget[]=[],selectionError:string|null=null;
 try{targets=ocrTargets(document.pages,selectPages(document.pages,mode,document.selectedPageId,range));}
 catch(error){selectionError=error instanceof Error?error.message:String(error);}
 const close=()=>{workflow.cancel();onClose();};
 const select=(index:number)=>{
  if(wordDirty||busy)return;setSelected(index);
  const position=visible.findIndex(item=>item.index===index);if(position>=0)setListPage(Math.floor(position/50));
 };
 const pickWord=(event:MouseEvent<SVGSVGElement>)=>{
  if(!draft||wordDirty||busy)return;const area=event.currentTarget.getBoundingClientRect();if(!area.width||!area.height)return;
  const x=(event.clientX-area.left)/area.width*draft.page.width,y=(event.clientY-area.top)/area.height*draft.page.height;
  let found=-1,size=Infinity;
  words.forEach((word,i)=>{const b=word.bounds;if(x>=b.x&&x<=b.x+b.width&&y>=b.y&&y<=b.y+b.height&&b.width*b.height<size){found=i;size=b.width*b.height;}});
  if(found>=0){setUncertainOnly(false);setListPage(Math.floor(found/50));setSelected(found);}
 };
 return <Modal className="ocr-dialog" title="Recognize text" description="Make scans searchable and copyable. Recognition runs locally on your computer." onClose={close}>
  {workflow.stage==='setup'?<div className="ocr-setup">
   <label>Pages<select value={mode} onChange={event=>setMode(event.target.value as typeof mode)}><option value="current">Current page</option><option value="all">All pages</option><option value="range">Page range</option></select></label>
   {mode==='range'&&<label>Page range<input value={range} onChange={event=>setRange(event.target.value)} placeholder="1, 3-5" aria-invalid={!!selectionError}/></label>}
   <fieldset><legend>Languages on the page</legend>{languages.map(([id,label])=><label className="ocr-check" key={id}><input type="checkbox" checked={chosen.includes(id)} onChange={event=>setChosen(current=>event.target.checked?[...current,id]:current.filter(item=>item!==id))}/>{label}</label>)}</fieldset>
   <p>Choose only the languages present. English is selected by default. Up to 100 distinct source pages can be recognized at once.</p>
   <p>Copies of the same original page update together, including copies outside your selected range. Their page numbers appear in review.</p>
   <p>Existing selectable text is preserved. PDF annotations, form fields and added editor text are excluded from recognition. Existing verified Folio OCR opens for review without running recognition again.</p>
   {selectionError&&<p className="error-note" role="alert">{selectionError}</p>}
   {!chosen.length&&<p>Choose at least one language.</p>}
   <p>{targets.length} distinct source page{targets.length===1?'':'s'} selected.</p>
  </div>:<>
   {workflow.stage==='recognizing'&&<div className="ocr-progress" role="status"><p>Page {workflow.progress.page} of {workflow.progress.total}: {workflow.progress.message}</p><progress max={1} value={workflow.progress.fraction} aria-label="Recognition progress"/></div>}
   {workflow.stage==='applying'&&<p role="status">Verifying and applying searchable text… Cancel discards any pending result.</p>}
   {workflow.stage!=='recognizing'&&draft&&<div className="ocr-review">
    <div className="ocr-preview-column">
     <label>Review page<select value={workflow.active} disabled={busy||wordDirty} onChange={event=>void workflow.view(Number(event.target.value))}>{workflow.drafts.map((draft,index)=><option key={index} value={index}>Page{draft.target.pageNumbers.length===1?'':'s'} {draft.target.pageNumbers.join(', ')}</option>)}</select></label>
     {cropBox&&<p>Full source shown for recognition. The dashed outline marks this page’s saved and printed crop. Shared copies may have different crops.</p>}
     <div className="ocr-preview-scroll">{workflow.preview?<div className="ocr-preview-image"><img src={workflow.preview} alt="Scanned page preview"/><svg aria-hidden="true" viewBox={`0 0 ${draft.page.width} ${draft.page.height}`} onClick={pickWord}>
      {cropBox&&<rect {...cropBox} fill="none" stroke="#2874b8" strokeWidth={2} strokeDasharray="6 4" vectorEffect="non-scaling-stroke" pointerEvents="none"/>}
      {Object.entries(paths).map(([kind,path])=><path key={kind} className={`ocr-box-${kind}`} d={path} vectorEffect="non-scaling-stroke"/>)}
      {word&&<path className="ocr-box-selected" d={rectangle(word.bounds)} vectorEffect="non-scaling-stroke"/>}
     </svg></div>:<p role="status">{workflow.previewLoading?'Rendering scan…':'Preview unavailable.'}</p>}</div>
     <p className="ocr-legend">Amber: score below 80 · Blue: recognized · Gray: existing text · Dashed: removed</p>
     <button className="button" disabled={busy||wordDirty||workflow.previewLoading} onClick={()=>void workflow.recognizeAgain(chosen)}>Recognize this page again</button>
     <p className="ocr-detail">Recognizing again replaces this page’s review draft.</p>
    </div>
    <div className="ocr-word-panel">
     <p>Corrections change searchable text. The scanned lettering stays unchanged.</p>
     <div className="ocr-word-summary"><span>{words.length} words · {excluded.size} already selectable · {uncertain.length} to review</span>
      <label className="ocr-check"><input type="checkbox" checked={uncertainOnly} disabled={busy||wordDirty} onChange={event=>{setUncertainOnly(event.target.checked);setListPage(0);}}/>Show only scores below 80</label>
      <button className="button" disabled={busy||wordDirty||!uncertain.length} onClick={()=>select(uncertain.find(index=>index>selected)??uncertain[0])}>Next uncertain word</button>
     </div>
     <div className="ocr-word-list" aria-label="Recognized words">{visible.slice(offset,offset+50).map(({word,index})=><button type="button" key={index} aria-pressed={index===selected} disabled={busy||wordDirty} className={excluded.has(index)?'existing':removed.has(index)?'removed':''} onClick={()=>select(index)}>
      <span>{word.text}</span><small>{excluded.has(index)?'Existing text':removed.has(index)?'Removed':`Score ${Math.round(word.confidence)}`}</small>
     </button>)}{!visible.length&&<p>{words.length?'No words match this filter.':'No text was recognized on this page.'}</p>}</div>
     {listPages>1&&<div className="ocr-list-pages"><button className="button" disabled={busy||wordDirty||listPage===0} onClick={()=>setListPage(p=>p-1)}>Previous words</button><span>{Math.min(listPage+1,listPages)} / {listPages}</span><button className="button" disabled={busy||wordDirty||listPage>=listPages-1} onClick={()=>setListPage(p=>p+1)}>More words</button></div>}
     {word&&<>
      <label>Recognized text<input value={value} disabled={busy||!editable} onChange={event=>setValue(event.target.value)}/></label>
      {excluded.has(selected)?<p>This is existing selectable text and is excluded from OCR changes.</p>:<>
       <p>Recognition score: {Math.round(word.confidence)}. This score is a review aid, not a probability of correctness.</p>
       <div className="ocr-word-actions"><button className="button" disabled={busy||!wordDirty} onClick={()=>workflow.update(draft=>editOcrWord(draft,selected,value))}>Save correction</button>
        <button className="button" disabled={busy||!wordDirty} onClick={()=>setValue(word.text)}>Reset correction</button>
        <button className="button" disabled={busy||wordDirty} onClick={()=>workflow.update(draft=>removeOcrWord(draft,selected,!removed.has(selected)))}>{removed.has(selected)?'Restore word':'Remove word'}</button></div>
       {wordDirty&&<p>Save or reset this correction before choosing another word or applying.</p>}
      </>}
     </>}
     <p>{workflow.changed} source page{workflow.changed===1?'':'s'} changed. Apply makes one undo step.</p>
    </div>
   </div>}
  </>}
  {workflow.error&&<p className="error-note" role="alert">{workflow.error}</p>}
  {workflow.notice&&<p role="status">{workflow.notice}</p>}
  <div className="dialog-actions"><button className="button" onClick={close}>Cancel</button>
   {workflow.stage==='setup'?<button className="button primary" disabled={!!selectionError||!chosen.length} onClick={()=>void workflow.start(targets,chosen)}>Start recognition</button>:workflow.stage!=='recognizing'&&<>
    <button className="button" disabled={busy} onClick={workflow.reset}>Start over</button>
    <button className="button primary" disabled={busy||workflow.previewLoading||wordDirty||!workflow.changed} onClick={()=>void workflow.apply().then(applied=>{if(applied)onClose();})}>Apply searchable text</button>
   </>}
  </div>
 </Modal>;
}
