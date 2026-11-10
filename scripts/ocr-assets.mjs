import {readFile,writeFile,mkdir,rename,rm,stat} from 'node:fs/promises';
import {join,dirname,resolve,relative,isAbsolute} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash,randomUUID} from 'node:crypto';

const defaultRoot=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const setupHint='Run npm run setup:ocr to prepare the verified local OCR assets.';
function under(root,path) {
  const target=resolve(root,path),rel=relative(root,target);
  if(!rel||rel==='..'||rel.startsWith(`..\\`)||rel.startsWith('../')||isAbsolute(rel))throw new Error(`Invalid OCR asset path: ${path}`);
  return target;
}
function verify(bytes,entry) {
  if(bytes.length!==entry.bytes)throw new Error(`${entry.path}: OCR asset size mismatch.`);
  if(hash(bytes)!==entry.sha256)throw new Error(`${entry.path}: OCR asset checksum mismatch.`);
  return bytes;
}
async function localBytes(path,entry) {
  const info=await stat(path);
  if(info.size!==entry.bytes)throw new Error(`${entry.path}: OCR asset size mismatch.`);
  return verify(await readFile(path),entry);
}
async function downloadBytes(entry,fetchImpl) {
  const response=await fetchImpl(entry.url,{signal:AbortSignal.timeout(120000),redirect:'error'});
  if(!response.ok)throw new Error(`${entry.path}: download HTTP ${response.status}.`);
  if(!response.body)throw new Error(`${entry.path}: empty download.`);
  const reader=response.body.getReader(),chunks=[];let length=0;
  try {
    while(true){
      const {done,value}=await reader.read();if(done)break;
      length+=value.length;
      if(length>entry.bytes)throw new Error(`${entry.path}: download size exceeds pinned size.`);
      chunks.push(value);
    }
  }catch(error){await reader.cancel().catch(()=>{});throw error;}
  finally{reader.releaseLock();}
  return verify(Buffer.concat(chunks,length),entry);
}
async function atomicFile(path,bytes) {
  await mkdir(dirname(path),{recursive:true});
  const temporary=`${path}.${randomUUID()}.partial`;
  try{await writeFile(temporary,bytes,{flag:'wx'});await rename(temporary,path);}
  finally{await rm(temporary,{force:true});}
}

/** Explicit setup may fetch fixed public models; offline validation never mutates or fetches. */
export async function prepareOcrAssets({root=defaultRoot,download=false,fetchImpl=fetch,manifest}={}) {
  manifest??=JSON.parse(await readFile(join(defaultRoot,'scripts/ocr-assets.json'),'utf8'));
  for(const [name,version] of Object.entries(manifest.packages)){
    const installed=JSON.parse(await readFile(under(root,`node_modules/${name}/package.json`),'utf8'));
    if(installed.version!==version)throw new Error(`OCR requires ${name}@${version}; run npm ci.`);
  }
  const output=join(root,'public/ocr'),prepared=[];
  for(const spec of manifest.files){
    const destination=under(output,spec.path);
    let source;
    if(spec.package)source=await localBytes(under(root,`node_modules/${spec.package}/${spec.file}`),spec);
    else if(spec.copy)source=await readFile(under(root,spec.copy));
    const entry=spec.copy?{...spec,bytes:source.length,sha256:hash(source)}:spec;
    let bytes;
    try{bytes=await localBytes(destination,entry);}
    catch(error){
      if(!download)throw new Error(`${entry.path}: ${error.message} ${setupHint}`);
      bytes=source??await downloadBytes(entry,fetchImpl);
    }
    prepared.push({entry,bytes,destination});
  }
  const published={schemaVersion:1,packages:manifest.packages,files:prepared.map(({entry})=>entry)};
  if(download){
    // Validate every input before publishing any asset; manifest is written last.
    for(const item of prepared)await atomicFile(item.destination,item.bytes);
    await atomicFile(join(output,'manifest.json'),JSON.stringify(published,null,2)+'\n');
  }else{
    let actual;
    try{actual=JSON.parse(await readFile(join(output,'manifest.json'),'utf8'));}catch{throw new Error(`OCR manifest is missing or invalid. ${setupHint}`);}
    if(JSON.stringify(actual)!==JSON.stringify(published))throw new Error(`OCR manifest does not match the pinned assets. ${setupHint}`);
  }
  return published;
}
