import {prepareOcrAssets} from './ocr-assets.mjs';
const args=process.argv.slice(2);
if(args.some(arg=>arg!=='--offline'))throw new Error('Usage: node scripts/setup-ocr.mjs [--offline]');
try{
  const result=await prepareOcrAssets({download:!args.includes('--offline')});
  console.log(`Verified ${result.files.length} local OCR assets (${args.includes('--offline')?'offline':'setup'}).`);
}catch(error){console.error(error.message);process.exitCode=1;}
