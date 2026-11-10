import {describe,it,expect} from 'vitest';
import {normalizeOcrResult,validateOcrRaster} from '../src/editor/ocr/result';

const word=(text='café',box={x0:2,y0:3,x1:32,y1:13})=>({text,confidence:91,bbox:box});
const raw=(words:unknown[]=[word()])=>({blocks:[{paragraphs:[{lines:[{words}]}]}]});
export function png(width=100,height=80):Uint8Array {
  const bytes=new Uint8Array(33);bytes.set([137,80,78,71,13,10,26,10]);
  const v=new DataView(bytes.buffer);v.setUint32(8,13);bytes.set([73,72,68,82],12);
  v.setUint32(16,width);v.setUint32(20,height);return bytes;
}
describe('OCR geometry and logical text',()=>{
  it('preserves accented/supplementary text, reading order and only useful fields',()=>{
    const result=normalizeOcrResult(raw([word('café'),word('niño'),word('𐐀')]),100,80);
    expect(result).toEqual({width:100,height:80,blocks:[{paragraphs:[{lines:[{words:[
      {text:'café',confidence:91,bounds:{x:2,y:3,width:30,height:10}},
      {text:'niño',confidence:91,bounds:{x:2,y:3,width:30,height:10}},
      {text:'𐐀',confidence:91,bounds:{x:2,y:3,width:30,height:10}},
    ]}]}]}]});
  });
  it('accepts blank pages and omits empty containers',()=>{
    expect(normalizeOcrResult({blocks:null},100,80).blocks).toEqual([]);
    expect(normalizeOcrResult(raw([]),100,80).blocks).toEqual([]);
  });
  it.each([null,{}, {blocks:'text'}, {blocks:[{}]},raw([null]),raw([{...word(),text:''}]),
    raw([{...word(),text:'bad\u0000word'}]),raw([{...word(),text:'bad\ud800'}]),
    raw([{...word(),confidence:NaN}]),raw([{...word(),confidence:101}]),
    raw([word('x',{x0:-1,y0:0,x1:5,y1:5})]),raw([word('x',{x0:0,y0:0,x1:101,y1:5})]),
    raw([word('x',{x0:5,y0:0,x1:5,y1:5})]),raw([word('x',{x0:0,y0:0,x1:5,y1:Infinity})]),
  ])('rejects malformed OCR without silently discarding words: %#',value=>{
    expect(()=>normalizeOcrResult(value,100,80)).toThrow(/OCR/);
  });
  it('enforces word, line and Unicode scalar budgets',()=>{
    expect(()=>normalizeOcrResult(raw(Array.from({length:20001},()=>word())),100,80)).toThrow(/limit/);
    expect(()=>normalizeOcrResult(raw([word('a'.repeat(100001))]),100,80)).toThrow(/limit/);
    expect(()=>normalizeOcrResult({blocks:[{paragraphs:[{lines:Array.from({length:10001},()=>({words:[]}))}]}]},100,80)).toThrow(/limit/);
    expect(normalizeOcrResult(raw([word('𐐀'.repeat(100000))]),100,80).blocks).toHaveLength(1);
  });
});
describe('OCR raster admission',()=>{
  it('accepts a matching PNG raster without changing caller bytes',()=>{
    const image={png:png(),width:100,height:80,dpi:300};
    expect(()=>validateOcrRaster(image)).not.toThrow();expect(image.png.byteLength).toBe(33);
  });
  it.each([
    {png:png(),width:101,height:80,dpi:300},
    {png:png(),width:100,height:80,dpi:NaN},
    {png:png(),width:100,height:80,dpi:601},
    {png:png(),width:100,height:80,dpi:71},
    {png:png(6001,1),width:6001,height:1,dpi:300},
    {png:png(5000,5000),width:5000,height:5000,dpi:300},
    {png:new Uint8Array(33),width:100,height:80,dpi:300},
    {png:new Uint8Array(32*1024*1024+1),width:100,height:80,dpi:300},
  ])('rejects mismatches, oversized buffers and unsupported declarations: %#',value=>{
    expect(()=>validateOcrRaster(value)).toThrow(/OCR/);
  });
});
