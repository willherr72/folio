import type {Point} from './geometry';

/** A PDF-space point at the viewport center; stable across page reordering. */
export interface ReadingLocation {pageId:string;zoom:number;point?:Point}
export interface NavigationHistory {back:ReadingLocation[];forward:ReadingLocation[]}
export interface NavigationRequest {pageId:string;revision:number;rect?:{x:number;y:number;width:number;height:number};point?:Point;origin?:ReadingLocation}
export type NavigationDirection='back'|'forward';
const LIMIT=100;
export function emptyNavigationHistory():NavigationHistory{return {back:[],forward:[]};}
function valid(view:ReadingLocation){return !!view.pageId&&Number.isFinite(view.zoom)&&view.zoom>=50&&view.zoom<=200&&(!view.point||(Number.isFinite(view.point.x)&&Number.isFinite(view.point.y)));}
export function sameReadingLocation(a:ReadingLocation,b:ReadingLocation):boolean {
 return a.pageId===b.pageId&&a.zoom===b.zoom&&(!a.point&&!b.point||!!a.point&&!!b.point&&Math.abs(a.point.x-b.point.x)<0.5&&Math.abs(a.point.y-b.point.y)<0.5);
}
function push(stack:ReadingLocation[],view:ReadingLocation):ReadingLocation[]{
 return stack.length&&sameReadingLocation(stack[stack.length-1],view)?stack:[...stack,view].slice(-LIMIT);
}
export function recordNavigation(history:NavigationHistory,origin:ReadingLocation,destination:ReadingLocation):NavigationHistory {
 if(!valid(origin)||!valid(destination)||sameReadingLocation(origin,destination))return history;
 return {back:push(history.back,origin),forward:[]};
}
export function canTravelNavigation(history:NavigationHistory,direction:NavigationDirection,pageIds:readonly string[]):boolean {
 const ids=new Set(pageIds);return history[direction].some(view=>ids.has(view.pageId)&&valid(view));
}
export function travelNavigation(history:NavigationHistory,current:ReadingLocation,direction:NavigationDirection,pageIds:readonly string[]):{history:NavigationHistory;target:ReadingLocation}|null {
 if(!valid(current))return null;
 const ids=new Set(pageIds),stack=history[direction];
 let identical:ReadingLocation|null=null;
 for(let i=stack.length-1;i>=0;i--){
  const target=stack[i];if(!ids.has(target.pageId)||!valid(target))continue;
  if(sameReadingLocation(target,current)){identical=target;continue;}
  const opposite=direction==='back'?'forward':'back';
  return {target,history:{...history,[direction]:stack.slice(0,i),[opposite]:push(history[opposite].filter(view=>ids.has(view.pageId)&&valid(view)),current)}};
 }
 // Manual scrolling can already have returned to the only remaining destination.
 // Consume that entry instead of leaving an enabled Back button stuck forever.
 if(identical){const opposite=direction==='back'?'forward':'back';return {target:identical,history:{...history,[direction]:[],[opposite]:push(history[opposite].filter(view=>ids.has(view.pageId)&&valid(view)),current)}};}
 return null;
}
