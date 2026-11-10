import type {StyledParagraphSelection} from "../../src/editor/types";
export function styledEditorFixture():StyledParagraphSelection{return {objectIndices:[0,1],text:"one two",runs:[{text:"one ",styleId:0},{text:"two",styleId:1}],styles:[
 {styleId:0,objectIndices:[0],fontName:"Courier",fontDisplayName:null,fontSize:12,color:"#000000",isEmbedded:false},
 {styleId:1,objectIndices:[1],fontName:"Helvetica-Bold",fontDisplayName:null,fontSize:20,color:"#882244",isEmbedded:false}],bounds:{x:20,y:30,width:200,height:100},alignment:"right",lineSpacing:1.6};}
