import {expect,it} from 'vitest';
import {frameDraft,fieldFrameRequest,type FormFrameAppearance} from '../src/editor/form-frame';
import type {EditorDocument,FormField,PagePlan} from '../src/editor/types';
const page:PagePlan={id:'p',sourceId:'source',pageIndex:0,width:300,height:400,rotation:0,overlays:[]};
const document={id:'d',name:'Document',pages:[page],selectedPageId:'p',selectedOverlayId:null} as EditorDocument;
const field={annotationIndex:2,kind:'text',value:{kind:'text',text:'Keep'},bounds:{x:20,y:30,width:100,height:30},visible:true} as FormField;
export const style:FormFrameAppearance={styleKey:'a'.repeat(64),width:0.5,style:'solid',border:{colorSpace:'cmyk',components:[1,0,0,0],preview:'#00ffff'},background:{colorSpace:'transparent',components:[],preview:null},supportedStyles:['solid','dashed','underline','beveled','inset']};
const request=(draft=frameDraft(style),source=style)=>fieldFrameRequest(document,page,field,source,draft);
it('keeps all Original settings absent and sends only a changed physical width',()=>{
 expect(()=>request()).toThrow(/change/i);
 for(const width of ['0','2.25','32'])expect(request({...frameDraft(style),width}).changes).toEqual({width:Number(width)});
 for(const width of ['',' ','-1','32.01','NaN','Infinity'])expect(()=>request({...frameDraft(style),width})).toThrow(/0.*32/);
});
it('preserves original palettes but permits deliberate RGB conversion and transparency',()=>{
 expect(request({...frameDraft(style),borderMode:'rgb',borderColor:'#00FFFF'}).changes).toEqual({border:{mode:'rgb',color:'#00ffff'}});
 expect(request({...frameDraft(style),borderMode:'transparent'}).changes).toEqual({border:{mode:'transparent'}});
 expect(request({...frameDraft(style),backgroundMode:'rgb',backgroundColor:'#112233'}).changes).toEqual({background:{mode:'rgb',color:'#112233'}});
 expect(()=>request({...frameDraft(style),borderMode:'rgb',borderColor:'red'})).toThrow(/RGB/);
 expect(()=>request({...frameDraft(style),backgroundMode:'transparent'})).toThrow(/change/i);
});
it('keeps an ambiguous Original style and limits choices to the inspected shape',()=>{
 const radio={...style,style:null,supportedStyles:['solid','dashed'] as const};
 const source={...radio,supportedStyles:[...radio.supportedStyles]};
 expect(frameDraft(source).style).toBe('original');
 expect(request({...frameDraft(source),width:'1'},source).changes).toEqual({width:1});
 expect(request({...frameDraft(source),style:'dashed'},source).changes).toEqual({style:'dashed'});
 expect(()=>request({...frameDraft(source),style:'underline'},source)).toThrow(/style/i);
});
it('restores transient strings on reset and preserves an unchanged source width above the edit limit',()=>{
 const source={...style,width:40};
 expect(request({...frameDraft(source),backgroundMode:'rgb',backgroundColor:'#123456'},source).changes).toEqual({background:{mode:'rgb',color:'#123456'}});
 expect(frameDraft(style)).toMatchObject({width:'0.5',style:'original',borderMode:'original',backgroundMode:'original'});
});
it.each(['text','checkbox','radio','combo','list'] as const)('accepts the native %s field kind',kind=>{
 expect(fieldFrameRequest(document,page,{...field,kind},style,{...frameDraft(style),width:'1'}).changes).toEqual({width:1});
});
