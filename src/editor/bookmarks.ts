import type {DocumentBookmark,EditorDocument,PagePlan,PdfValue} from "./types";
import {uniqueId} from "./model";
export const MAX_BOOKMARKS=4000;
const nameEquals=(bytes:number[],text:string)=>bytes.length===text.length&&bytes.every((b,i)=>b===text.charCodeAt(i));
const entry=(value:Extract<PdfValue,{kind:"dictionary"}>,key:string)=>value.value.find(([k])=>nameEquals(k,key))?.[1];
const isGoTo=(v:Extract<PdfValue,{kind:"dictionary"}>)=>{const s=entry(v,"S");return s?.kind==="name"&&nameEquals(s.value,"GoTo");};
const destinationPage=(value?:PdfValue)=>value?.kind==="array"&&value.value[0]?.kind==="page"?value.value[0].value:null;
export function bookmarkPage(bookmark:DocumentBookmark):string|null {
  return destinationPage(bookmark.destination)??(bookmark.action?.kind==="dictionary"&&isGoTo(bookmark.action)?destinationPage(entry(bookmark.action,"D")):null);
}
function mapValue(value:PdfValue,map:Map<string,string>):PdfValue {
  if(value.kind==="page"){const id=map.get(value.value);return id?{kind:"page",value:id}:{kind:"null"};}
  if(value.kind==="array")return {...value,value:value.value.map(v=>mapValue(v,map))};
  if(value.kind==="dictionary"){
    const mapped:Extract<PdfValue,{kind:"dictionary"}>={kind:"dictionary",value:value.value.map(([k,v])=>[k,mapValue(v,map)])};
    if(isGoTo(mapped)&&!destinationPage(entry(mapped,"D")))return {kind:"null"};
    mapped.value=mapped.value.flatMap(([k,v])=>{
      if(!nameEquals(k,"Next"))return [[k,v] as [number[],PdfValue]];
      if(v.kind==="array")v={...v,value:v.value.filter(item=>item.kind!=="null")};
      return v.kind==="null"||(v.kind==="array"&&!v.value.length)?[]:[[k,v] as [number[],PdfValue]];
    });return mapped;
  }
  if(value.kind==="stream")return {...value,value:{...value.value,dictionary:value.value.dictionary.map(([k,v])=>[k,mapValue(v,map)])}};
  return value;
}
function remap(items:DocumentBookmark[],map:Map<string,string>):DocumentBookmark[] {
  const result:DocumentBookmark[]=[];
  for(const item of [...items].reverse()){
    const page=bookmarkPage(item),missing=page!==null&&!map.has(page);
    const hasChildren=(result.at(-1)?.level??-1)>item.level;
    if(missing&&!hasChildren)continue;
    const next={...item};
    if(item.destination){next.destination=mapValue(item.destination,map);if(!destinationPage(next.destination))delete next.destination;}
    if(item.action){next.action=mapValue(item.action,map);if(next.action.kind==="null")delete next.action;}
    result.push(next);
  }return result.reverse();
}
export function importBookmarks(items:DocumentBookmark[]|undefined,pages:PagePlan[]):DocumentBookmark[] {
  return remap(items??[],new Map(pages.map((p,i)=>[String(i),p.id]))).map(b=>({...b,id:uniqueId("bookmark")}));
}
export function filterBookmarks(items:DocumentBookmark[],pages:PagePlan[]):DocumentBookmark[] {
  return remap(items,new Map(pages.map(p=>[p.id,p.id])));
}
function title(text:string):string {
  const value=text.trim();
  if(!value)throw new Error("Enter a bookmark title.");
  if(new TextEncoder().encode(value).length>4096)throw new Error("Bookmark titles must fit within 4096 UTF-8 bytes.");
  return value;
}
function fit(pageId:string):PdfValue {return {kind:"array",value:[{kind:"page",value:pageId},{kind:"name",value:[70,105,116]}]};}
export function branchEnd(items:DocumentBookmark[],index:number):number {let end=index+1;while(end<items.length&&items[end].level>items[index].level)end++;return end;}
export function addBookmark(document:EditorDocument,text:string,pageId:string,parentId?:string):EditorDocument {
  const items=document.bookmarks??[];
  if(items.length>=MAX_BOOKMARKS)throw new Error("A document can contain at most 4000 bookmarks.");
  if(!document.pages.some(p=>p.id===pageId))throw new Error("The destination page has changed.");
  const parent=parentId?items.findIndex(b=>b.id===parentId):-1;
  if(parentId&&parent<0)throw new Error("The parent bookmark has changed.");
  const level=parent>=0?items[parent].level+1:0;
  if(level>=32)throw new Error("Bookmarks can have at most 32 levels.");
  const bookmark={id:uniqueId("bookmark"),title:title(text),level,destination:fit(pageId)};
  const next=[...items];next.splice(parent>=0?branchEnd(items,parent):items.length,0,bookmark);
  return {...document,bookmarks:next};
}
export function editBookmark(document:EditorDocument,id:string,update:{title?:string;pageId?:string}):EditorDocument {
  if(update.pageId&&!document.pages.some(p=>p.id===update.pageId))throw new Error("The destination page has changed.");
  return {...document,bookmarks:(document.bookmarks??[]).map(b=>{
    if(b.id!==id)return b;
    const next={...b,...(update.title!==undefined?{title:title(update.title)}:{})};
    if(update.pageId){next.destination=fit(update.pageId);delete next.action;}
    return next;
  })};
}
export function removeBookmark(document:EditorDocument,id:string):EditorDocument {
  const items=document.bookmarks??[],index=items.findIndex(b=>b.id===id);
  if(index<0)return document;const end=branchEnd(items,index);
  return {...document,bookmarks:[...items.slice(0,index),...items.slice(end)]};
}
export function moveBookmark(document:EditorDocument,id:string,direction:-1|1):EditorDocument {
  const items=document.bookmarks??[],index=items.findIndex(b=>b.id===id);if(index<0)return document;
  const end=branchEnd(items,index),level=items[index].level;
  if(direction===1){
    if(end>=items.length||items[end].level!==level)return document;
    const nextEnd=branchEnd(items,end);return {...document,bookmarks:[...items.slice(0,index),...items.slice(end,nextEnd),...items.slice(index,end),...items.slice(nextEnd)]};
  }
  let previous=index-1;while(previous>=0&&items[previous].level>level)previous--;
  if(previous<0||items[previous].level!==level)return document;
  return {...document,bookmarks:[...items.slice(0,previous),...items.slice(index,end),...items.slice(previous,index),...items.slice(end)]};
}
