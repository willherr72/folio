import type {ContentRemovalRaster} from './text-removal';

export interface ValidatedContentRaster extends Omit<ContentRemovalRaster,'sourcePng'|'candidatePng'|'disclosurePng'> {
 sourcePng:Uint8Array<ArrayBuffer>;candidatePng:Uint8Array<ArrayBuffer>|null;disclosurePng:Uint8Array<ArrayBuffer>|null;
}
function check(value:unknown):asserts value {if(!value)throw new Error('The content-removal page preview is invalid.');}
/** Validate transport metadata before creating any display URLs. Image loading is a separate gate. */
export function validateContentRaster(value:ContentRemovalRaster,expected:{sourceId:string;previewId:string|null;candidateId:string|null;pageIndex:number;pageWidth:number;pageHeight:number;requestedWidth:number}):ValidatedContentRaster {
 check(value&&typeof value==='object');
 // PageInfo serializes f32 using its shortest decimal. Raster metadata promotes
 // the same native value to f64. Reconstruct the original value before binding.
 const pageWidth=Math.fround(expected.pageWidth),pageHeight=Math.fround(expected.pageHeight);
 check([pageWidth,pageHeight].every(n=>Number.isFinite(n)&&n>=1&&n<=20000)&&Number.isInteger(expected.requestedWidth)&&expected.requestedWidth>=64&&expected.requestedWidth<=1200);
 const scale=Math.min(expected.requestedWidth/pageWidth,2400/pageHeight);
 const width=Math.max(1,Math.round(pageWidth*scale)),height=Math.max(1,Math.round(pageHeight*scale));
 check(value.sourceId===expected.sourceId&&value.previewId===expected.previewId&&value.candidateId===expected.candidateId&&value.pageIndex===expected.pageIndex);
 check(value.pageWidth===pageWidth&&value.pageHeight===pageHeight&&value.width===width&&value.height===height&&width<=1200&&height<=2400&&width*height<=2_880_000);
 let remaining=16*1024*1024;
 const png=(input:number[]):Uint8Array<ArrayBuffer>=>{
  check(Array.isArray(input)&&input.length>=45&&input.length<=remaining);remaining-=input.length;
  check(input.every(n=>Number.isInteger(n)&&n>=0&&n<=255));
  const bytes=new Uint8Array(input),view=new DataView(bytes.buffer);
  check([137,80,78,71,13,10,26,10].every((n,i)=>bytes[i]===n)&&view.getUint32(8)===13&&[73,72,68,82].every((n,i)=>bytes[12+i]===n));
  check(view.getUint32(16)===width&&view.getUint32(20)===height);
  return bytes;
 };
 const sourcePng=png(value.sourcePng);
 let candidatePng:Uint8Array<ArrayBuffer>|null=null,disclosurePng:Uint8Array<ArrayBuffer>|null=null;
 if(expected.previewId){check(value.candidatePng&&value.disclosurePng);candidatePng=png(value.candidatePng);disclosurePng=png(value.disclosurePng);}
 else check(value.candidatePng===null&&value.disclosurePng===null);
 // Keep only bounded, validated fields. Byte arrays are owned copies.
 return Object.freeze({sourceId:expected.sourceId,previewId:expected.previewId,candidateId:expected.candidateId,pageIndex:expected.pageIndex,
  pageWidth,pageHeight,width,height,sourcePng,candidatePng,disclosurePng});
}
