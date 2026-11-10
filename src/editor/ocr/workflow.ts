import type {FolioAdapter} from '../adapter';
import {commit} from '../model';
import {pageAllows} from '../protection';
import type {DocumentSession} from '../workspace';
import type {EditorDocument,PagePlan,SourceReplacement} from '../types';
import type {OcrBounds,OcrPage,OcrUpdate,OcrWord} from './types';
import {copyOcrPage} from './result';

export interface OcrTarget {page:PagePlan;pageNumbers:number[]}
export interface OcrDraft {target:OcrTarget;page:OcrPage;baseline:OcrPage|null;excluded:number[];removed:number[];preserveOwnedOrientation:boolean}
export const ocrWords=(page:OcrPage):OcrWord[]=>page.blocks.flatMap(b=>b.paragraphs.flatMap(p=>p.lines.flatMap(l=>l.words)));
const key=(page:PagePlan)=>JSON.stringify([page.sourceId,page.pageIndex]);
const aborted=()=>new DOMException('OCR canceled.','AbortError');
function check(signal:AbortSignal):void {if(signal.aborted)throw aborted();}

export function ocrTargets(pages:PagePlan[],selected:PagePlan[]):OcrTarget[] {
  const result:OcrTarget[]=[],seen=new Set<string>();
  for(const page of selected){
    if(!pages.includes(page))throw new Error('The selected page is no longer in this document.');
    if(!pageAllows(page,'editContent')||!pageAllows(page,'copyContent'))throw new Error('Recognizing text requires permission to edit and copy PDF content.');
    if(![0,90,180,270].includes(page.rotation))throw new Error('Unsupported page rotation.');
    const id=key(page);if(seen.has(id))continue;seen.add(id);
    result.push({page,pageNumbers:pages.flatMap((p,i)=>key(p)===id?[i+1]:[])});
    if(result.length>100)throw new Error('Choose at most 100 distinct source pages per recognition job.');
  }
  if(!result.length)throw new Error('Choose at least one page.');return result;
}

/** Match native mutual-center overlap with 0.5-pixel tolerance and bounded work. */
function excludedWords(page:OcrPage,bounds:OcrBounds[]):number[] {
  if(bounds.length>250_000)throw new Error('Too many existing text bounds.');
  const grid=Array.from({length:4096},()=>[] as number[]),broad:number[]=[];
  const cell=(x:number,y:number)=>[Math.max(0,Math.min(63,Math.floor(x/page.width*64))),Math.max(0,Math.min(63,Math.floor(y/page.height*64)))];
  bounds.forEach((b,i)=>{
    if(![b.x,b.y,b.width,b.height].every(Number.isFinite)||b.width<=0||b.height<=0)throw new Error('Invalid existing text bounds.');
    const [x0,y0]=cell(b.x-0.5,b.y-0.5),[x1,y1]=cell(b.x+b.width+0.5,b.y+b.height+0.5);
    if((x1-x0+1)*(y1-y0+1)>64){broad.push(i);return;}
    for(let y=y0;y<=y1;y++)for(let x=x0;x<=x1;x++)grid[y*64+x].push(i);
  });
  const excluded:number[]=[];let checks=0;
  ocrWords(page).forEach((word,index)=>{
    const wordBox=word.bounds,x=wordBox.x+wordBox.width/2,y=wordBox.y+wordBox.height/2;
    const [x0,y0]=cell(wordBox.x-0.5,wordBox.y-0.5),[x1,y1]=cell(wordBox.x+wordBox.width+0.5,wordBox.y+wordBox.height+0.5);
    checks+=(x1-x0+1)*(y1-y0+1);
    if(checks>5_000_000)throw new Error('Existing text overlap checks exceed the supported limit.');
    const hit=(list:number[])=>{
      for(const i of list){
        if(++checks>5_000_000)throw new Error('Existing text overlap checks exceed the supported limit.');
        const b=bounds[i],cx=b.x+b.width/2,cy=b.y+b.height/2;
        if((x>=b.x-0.5&&x<=b.x+b.width+0.5&&y>=b.y-0.5&&y<=b.y+b.height+0.5)
          ||(cx>=wordBox.x-0.5&&cx<=wordBox.x+wordBox.width+0.5&&cy>=wordBox.y-0.5&&cy<=wordBox.y+wordBox.height+0.5))return true;
      }
      return false;
    };
    for(let cy=y0;cy<=y1;cy++)for(let cx=x0;cx<=x1;cx++)if(hit(grid[cy*64+cx])){excluded.push(index);return;}
    if(hit(broad))excluded.push(index);
  });return excluded;
}
export function createOcrDraft(target:OcrTarget,page:OcrPage,baseline:OcrPage|null,exclusions:OcrBounds[],preserveOwnedOrientation=!!baseline):OcrDraft {
  const copy=copyOcrPage(page),before=baseline?copyOcrPage(baseline):null;
  if(before&&(copy.width!==before.width||copy.height!==before.height))throw new Error('OCR review dimensions changed.');
  return {target,page:copy,baseline:before,excluded:excludedWords(copy,exclusions),removed:[],preserveOwnedOrientation};
}
export function editOcrWord(draft:OcrDraft,index:number,text:string):OcrDraft {
  if(draft.excluded.includes(index))throw new Error('Existing native text cannot be changed in OCR review.');
  const page=copyOcrPage(draft.page),word=ocrWords(page)[index];if(!word)throw new Error('Word no longer exists.');
  word.text=text;return {...draft,page:copyOcrPage(page)};
}
export function removeOcrWord(draft:OcrDraft,index:number,remove:boolean):OcrDraft {
  if(!ocrWords(draft.page)[index])throw new Error('Word no longer exists.');
  return {...draft,removed:remove?[...new Set([...draft.removed,index])]:draft.removed.filter(i=>i!==index)};
}
export function acceptedOcrPage(draft:OcrDraft):OcrPage {
  const page=copyOcrPage(draft.page),omitted=new Set([...draft.excluded,...draft.removed]);let index=0;
  for(const block of page.blocks)for(const paragraph of block.paragraphs)for(const line of paragraph.lines)line.words=line.words.filter(()=>!omitted.has(index++));
  return copyOcrPage(page);
}
export function ocrDraftChanged(draft:OcrDraft):boolean {
  const baseline=draft.baseline??{width:draft.page.width,height:draft.page.height,blocks:[]};
  return JSON.stringify(acceptedOcrPage(draft))!==JSON.stringify(baseline);
}
export function commitOcrReplacements(session:DocumentSession,expected:EditorDocument,replacements:SourceReplacement[]):DocumentSession {
  if(session.history.present!==expected)throw new Error('The document changed during recognition. Reopen recognition and try again.');
  if(!replacements.length)throw new Error('No OCR source replacements.');
  const sources=new Set<string>(),ids=new Set(session.sourceIds);
  for(const {sourceId,document} of replacements){
    if(sources.has(sourceId)||!expected.pages.some(p=>p.sourceId===sourceId))throw new Error('OCR source is missing or duplicated.');
    if(!document.id||ids.has(document.id))throw new Error('OCR must create a new source version.');
    sources.add(sourceId);ids.add(document.id);
    for(const page of expected.pages.filter(p=>p.sourceId===sourceId)){
      const size=document.pages[page.pageIndex];
      if(!size||![size.width,size.height].every(Number.isFinite)||Math.abs(size.width-page.width)>0.01||Math.abs(size.height-page.height)>0.01)throw new Error('OCR changed page dimensions or omitted a page.');
      if((page.protection?.documentId??null)!==(document.protection?.documentId??null))throw new Error('OCR changed the document protection context.');
    }
  }
  const lookup=new Map(replacements.map(r=>[r.sourceId,r.document]));
  return {...session,sourceIds:[...ids],history:commit(session.history,current=>({...current,selectedOverlayId:null,
    pages:current.pages.map(page=>{const info=lookup.get(page.sourceId);return info?{...page,sourceId:info.id,contentSourceId:undefined,protection:info.protection}:page;}),
  }))};
}

/** A candidate belongs to this operation until the synchronous workspace commit accepts it. */
export async function applyOcrDrafts(adapter:FolioAdapter,drafts:OcrDraft[],knownIds:ReadonlySet<string>,signal:AbortSignal,
  adopt:(replacements:SourceReplacement[])=>boolean):Promise<boolean> {
  if(!adapter.applyOcr)throw new Error('PDF OCR is unavailable in this adapter.');check(signal);
  const existing=new Set(knownIds),groups=new Map<string,OcrUpdate[]>();let count=0,words=0,scalars=0;
  const pages=new Set<string>();
  for(const draft of drafts){
    if(!ocrDraftChanged(draft))continue;
    const id=key(draft.target.page);if(pages.has(id))throw new Error('An OCR source page appears twice.');pages.add(id);
    if(++count>100)throw new Error('Choose at most 100 distinct source pages.');
    const page=acceptedOcrPage(draft),selected=ocrWords(page);words+=selected.length;
    for(const word of selected)scalars+=[...word.text].length;
    if(words>500_000||scalars>2_000_000)throw new Error('OCR job exceeds 500,000 words or 2,000,000 characters.');
    const {sourceId,pageIndex,rotation}=draft.target.page,updates=groups.get(sourceId)??[];
    updates.push({pageIndex,rotation,page,preserveOwnedOrientation:draft.preserveOwnedOrientation});groups.set(sourceId,updates);
  }
  if(!groups.size)return false;
  const replacements:SourceReplacement[]=[],owned=new Set<string>();let adopted=false;
  try {
    for(const [sourceId,updates] of groups){
      check(signal);const document=await adapter.applyOcr(sourceId,updates);
      if(!document.id||existing.has(document.id)||owned.has(document.id))throw new Error('OCR must create a new source version.');
      owned.add(document.id);replacements.push({sourceId,document});check(signal);
    }
    check(signal);if(!adopt(replacements))throw new Error('The document changed while OCR was being applied.');
    adopted=true;return true;
  }finally{
    if(!adopted)await Promise.allSettled([...owned].map(id=>adapter.closeDocument(id)));
  }
}
