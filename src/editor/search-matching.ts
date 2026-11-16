import unicode from "../shared/search-unicode.json";
import type {PageSearchOptions} from "./types";

export const DEFAULT_SEARCH_OPTIONS:Readonly<PageSearchOptions>=Object.freeze({matchCase:false,wholeWords:false});
const folds=new Map(unicode.folds as [number,string][]);
const encoder=new TextEncoder();
const whitespace=/^[\u0009-\u000d\u0020\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000\ufeff]$/u;

function wordCode(cp:number|undefined):boolean {
 if(cp===undefined)return false;
 let low=0,high=unicode.wordRanges.length;
 while(low<high){const mid=(low+high)>>>1;const [start,end]=unicode.wordRanges[mid];if(cp<start)high=mid;else if(cp>end)low=mid+1;else return true;}
 return false;
}
function previous(value:string,offset:number):number {
 if(offset<=0)return -1;
 const last=value.charCodeAt(offset-1);
 return last>=0xdc00&&last<=0xdfff&&offset>1&&value.charCodeAt(offset-2)>=0xd800&&value.charCodeAt(offset-2)<=0xdbff?offset-2:offset-1;
}
function wordAt(value:string,offset:number):boolean {
 if(offset<0||offset>=value.length)return false;
 const cp=value.codePointAt(offset)!;
 if(wordCode(cp))return true;
 return (cp===0x27||cp===0x2019)&&wordCode(value.codePointAt(previous(value,offset)))&&wordCode(value.codePointAt(offset+1));
}
function normalize(parts:Iterable<string>,matchCase:boolean){
 const pieces:string[]=[],glyphs:number[]=[],edges=new Set([0]);let length=0,glyph=0,space=false;
 for(const part of parts){
  for(const scalar of part){
   let piece:string;
   if(whitespace.test(scalar)){if(!length||space)continue;piece=" ";space=true;}
   else{piece=matchCase?scalar:folds.get(scalar.codePointAt(0)!)??scalar;space=false;}
   edges.add(length);pieces.push(piece);
   for(let i=0;i<piece.length;i++)glyphs.push(glyph);
   length+=piece.length;edges.add(length);
  }
  glyph++;
 }
 if(space){pieces.pop();glyphs.pop();edges.delete(length);}
 return {value:pieces.join(""),glyphs,edges};
}

export function normalizeSearchQuery(query:string,options:PageSearchOptions=DEFAULT_SEARCH_OPTIONS):string {
 if(query.length>1024||encoder.encode(query).length>1024)throw new Error("Search text is too long. Use at most 1,024 UTF-8 bytes.");
 for(const scalar of query){const cp=scalar.codePointAt(0)!;if(cp>=0xd800&&cp<=0xdfff)throw new Error("Use well-formed Unicode search text.");}
 return normalize([query],options.matchCase).value;
}

/** Original glyph indices; offset is only a normalized UTF16 navigation identity. */
export function* searchCharacterRanges(parts:Iterable<string>,query:string,options:PageSearchOptions=DEFAULT_SEARCH_OPTIONS):Generator<{start:number;end:number;offset:number}> {
 const needle=normalizeSearchQuery(query,options);if(!needle)return;
 const text=normalize(parts,options.matchCase);let from=0;
 while(from<text.value.length){
  const offset=text.value.indexOf(needle,from);if(offset<0)return;
  const end=offset+needle.length;
  const accepted=text.edges.has(offset)&&text.edges.has(end)&&(!options.wholeWords||
   (wordAt(text.value,offset)&&wordAt(text.value,previous(text.value,end))&&!wordAt(text.value,previous(text.value,offset))&&!wordAt(text.value,end)));
  if(accepted){yield {start:text.glyphs[offset],end:text.glyphs[end-1]+1,offset};from=end;}
  else from=offset+(text.value.codePointAt(offset)!>0xffff?2:1);
 }
}
