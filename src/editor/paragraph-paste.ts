/** Stage clipboard text without changing the single-line input or PDF. */
import type {StyledEditorRange} from "./styled-paragraph-editor";

export interface ParagraphPaste {
 readonly replacement:string;
 readonly selection:StyledEditorRange;
}
const utf8=new TextEncoder();
const graphemes=new Intl.Segmenter(undefined,{granularity:"grapheme"});
const controls=/[\u0000-\u0009\u000b-\u001f\u007f-\u009f]/;
const fail=(message:string):never=>{throw new Error(message);};

export function prepareParagraphPaste(text:string,selection:Readonly<{start:number;end:number}>,clipboard:string):ParagraphPaste {
 if(typeof text!=="string"||typeof clipboard!=="string"||text.length>4096||clipboard.length>8192)fail("The pasted paragraph is too long. Use less text.");
 if(!selection||!Number.isSafeInteger(selection.start)||!Number.isSafeInteger(selection.end)||selection.start<0||selection.end<selection.start||selection.end>text.length)fail("Select the text to replace before pasting.");
 const boundaries=new Set([0,text.length,...Array.from(graphemes.segment(text),part=>part.index)]);
 if(!boundaries.has(selection.start)||!boundaries.has(selection.end))fail("Select whole characters before pasting.");
 const inserted=clipboard.replace(/\r\n?/g,"\n");
 const replacement=text.slice(0,selection.start)+inserted+text.slice(selection.end);
 if(replacement.length>4096||utf8.encode(replacement).length>4096)fail("The pasted paragraph is too long. Use less text.");
 if(controls.test(replacement))fail("Use plain text without tabs or control characters.");
 for(const character of replacement){const cp=character.codePointAt(0)!;if(cp>=0xd800&&cp<=0xdfff)fail("The pasted text contains an incomplete character.");}
 if(!replacement.trim())fail("Enter replacement text.");
 const insertedEnd=selection.start+inserted.length;
 // A trailing joiner or regional indicator can merge with the existing
 // suffix. Keep a collapsed caret after that whole resulting character.
 const caret=[...Array.from(graphemes.segment(replacement),part=>part.index),replacement.length].find(at=>at>=insertedEnd)!;
 return Object.freeze({replacement,selection:Object.freeze({start:caret,end:caret,direction:"forward" as const})});
}
