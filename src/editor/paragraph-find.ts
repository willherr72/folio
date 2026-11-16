export interface ParagraphMatch {readonly start:number;readonly end:number}
export interface ParagraphFindOptions {readonly matchCase:boolean;readonly wholeWords:boolean}
const graphemes=new Intl.Segmenter(undefined,{granularity:"grapheme"});
const words=new Intl.Segmenter(undefined,{granularity:"word"});
const encoder=new TextEncoder();
/** Literal draft text only; no regex/replacement expansion or HTML. */
export function paragraphFindText(text:string):string {
 if(typeof text!=="string"||text.length>8192)throw new Error("Use at most 4096 UTF-8 bytes of text.");
 text=text.replace(/\r\n?/g,"\n");
 if(encoder.encode(text).length>4096)throw new Error("Use at most 4096 UTF-8 bytes of text.");
 if(/[\u0000-\u0009\u000b-\u001f\u007f-\u009f]/.test(text))throw new Error("Use plain text and line breaks.");
 for(const c of text){const cp=c.codePointAt(0)!;if(cp>=0xd800&&cp<=0xdfff)throw new Error("Use well-formed Unicode text.");}
 return text;
}
export function findParagraphMatches(text:string,query:string,options:ParagraphFindOptions):readonly ParagraphMatch[]{
 query=paragraphFindText(query);if(!query)return Object.freeze([]);
 if(text.length>4096||encoder.encode(text).length>4096)throw new Error("Use a bounded paragraph draft.");
 const edges=new Set([0,text.length,...Array.from(graphemes.segment(text),part=>part.index)]);
 const segments=options.wholeWords?Array.from(words.segment(text)):[];
 const starts=new Set(segments.filter(part=>part.isWordLike).map(part=>part.index));
 const ends=new Set(segments.filter(part=>part.isWordLike).map(part=>part.index+part.segment.length));
 const expression=new RegExp(query.replace(/[.*+?^${}()|[\]\\]/g,"\\$&"),options.matchCase?"gu":"giu");
 const matches:ParagraphMatch[]=[];
 let match:RegExpExecArray|null;
 while((match=expression.exec(text))){
  const start=match.index,end=start+match[0].length;
  if(edges.has(start)&&edges.has(end)&&(!options.wholeWords||(starts.has(start)&&ends.has(end))))matches.push(Object.freeze({start,end}));
  // Only accepted matches consume their full range. A rejected candidate may
  // overlap a later whole-word/grapheme-safe start.
  else expression.lastIndex=start+(text.codePointAt(start)!>0xffff?2:1);
 }
 return Object.freeze(matches);
}
export function nextParagraphMatch(matches:readonly ParagraphMatch[],selection:ParagraphMatch,direction:1|-1):ParagraphMatch|null{
 if(!matches.length)return null;
 const selected=matches.findIndex(match=>match.start===selection.start&&match.end===selection.end);
 if(selected>=0)return matches[(selected+direction+matches.length)%matches.length];
 return direction===1?matches.find(match=>match.start>=selection.end)??matches[0]:[...matches].reverse().find(match=>match.end<=selection.start)??matches.at(-1)!;
}
