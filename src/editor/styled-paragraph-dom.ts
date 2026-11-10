import type {StyledEditorRange} from "./styled-paragraph-editor";
export function styledDomOffset(root:HTMLElement,node:Node,offset:number):number {
 if(node!==root&&!root.contains(node))throw new Error("The text selection left the paragraph.");
 const range=document.createRange();range.selectNodeContents(root);range.setEnd(node,offset);return range.toString().length;
}
export function styledDomSelection(root:HTMLElement):StyledEditorRange|null {
 const selection=root.ownerDocument.getSelection();if(!selection?.anchorNode||!selection.focusNode)return null;
 if(!root.contains(selection.anchorNode)||!root.contains(selection.focusNode))return null;
 const a=styledDomOffset(root,selection.anchorNode,selection.anchorOffset),b=styledDomOffset(root,selection.focusNode,selection.focusOffset);
 return {start:Math.min(a,b),end:Math.max(a,b),direction:a>b?"backward":"forward"};
}
function point(root:HTMLElement,offset:number):[Node,number]{
 const walker=root.ownerDocument.createTreeWalker(root,NodeFilter.SHOW_TEXT);let node:Node|null;
 while((node=walker.nextNode())){const length=node.textContent?.length??0;if(offset<=length)return [node,offset];offset-=length;}
 if(offset!==0)throw new Error("The paragraph caret is outside the draft.");return [root,root.childNodes.length];
}
export function restoreStyledDomSelection(root:HTMLElement,range:StyledEditorRange):void {
 const selection=root.ownerDocument.getSelection();if(!selection)return;const start=point(root,range.start),end=point(root,range.end);
 if(selection.setBaseAndExtent){const [anchor,focus]=range.direction==="backward"?[end,start]:[start,end];selection.setBaseAndExtent(...anchor,...focus);}
 else{const selected=root.ownerDocument.createRange();selected.setStart(...start);selected.setEnd(...end);selection.removeAllRanges();selection.addRange(selected);}
}
