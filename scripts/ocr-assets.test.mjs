import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, writeFile, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
import {prepareOcrAssets} from './ocr-assets.mjs';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
async function fixture(t) {
  const root = await mkdtemp(join(tmpdir(), 'folio-ocr-assets-'));
  t.after(() => rm(root, {recursive:true, force:true}));
  await mkdir(join(root,'node_modules/engine'),{recursive:true});
  await writeFile(join(root,'node_modules/engine/package.json'),JSON.stringify({version:'1.0.0'}));
  await writeFile(join(root,'node_modules/engine/worker.js'),'local worker');
  const model = Buffer.from('local model');
  const manifest = {packages:{engine:'1.0.0'},files:[
    {path:'upstream/worker.js',package:'engine',file:'worker.js',bytes:12,sha256:digest('local worker')},
    {path:'models/eng.traineddata',url:'https://example.invalid/fixed-model',bytes:model.length,sha256:digest(model)},
  ]};
  return {root,manifest,model};
}

test('offline preparation fails usefully without making network requests',async t=>{
  const f=await fixture(t);let calls=0;
  await assert.rejects(prepareOcrAssets({...f,download:false,fetchImpl:()=>{calls++;}}),/setup:ocr/);
  assert.equal(calls,0);
});
test('verified setup is repeatable offline and publishes a complete manifest',async t=>{
  const f=await fixture(t);let calls=0;
  const fetchImpl=async()=>{calls++;return new Response(f.model);};
  await prepareOcrAssets({...f,download:true,fetchImpl});
  await prepareOcrAssets({...f,download:false,fetchImpl});
  assert.equal(calls,1);
  const published=JSON.parse(await readFile(join(f.root,'public/ocr/manifest.json'),'utf8'));
  assert.equal(published.files.length,2);
  assert.equal(published.files[1].sha256,digest(f.model));
  assert.equal(await readFile(join(f.root,'public/ocr/upstream/worker.js'),'utf8'),'local worker');
});
test('offline checking detects modified assets even when manifest exists',async t=>{
  const f=await fixture(t);
  await prepareOcrAssets({...f,download:true,fetchImpl:async()=>new Response(f.model)});
  await writeFile(join(f.root,'public/ocr/models/eng.traineddata'),'altered');
  await assert.rejects(prepareOcrAssets({...f,download:false}),/models\/eng.*setup:ocr/);
});
test('wrong installed version fails before downloading',async t=>{
  const f=await fixture(t);f.manifest.packages.engine='2.0.0';
  await assert.rejects(prepareOcrAssets({...f,download:true,fetchImpl:()=>assert.fail('network')}),/engine.*2.0.0/);
});
test('corrupt upstream package fails rather than repinning it',async t=>{
  const f=await fixture(t);
  await writeFile(join(f.root,'node_modules/engine/worker.js'),'wrong worker');
  await assert.rejects(prepareOcrAssets({...f,download:true}),/worker.js.*checksum/);
});
test('bad download hash cannot publish a prepared manifest',async t=>{
  const f=await fixture(t);
  await assert.rejects(prepareOcrAssets({...f,download:true,fetchImpl:async()=>new Response('wrong model')}),/checksum/);
  await assert.rejects(readFile(join(f.root,'public/ocr/manifest.json')),/ENOENT/);
});
test('oversized streaming download is canceled before publication',async t=>{
  const f=await fixture(t);let canceled=false;
  const stream=new ReadableStream({pull(c){c.enqueue(new Uint8Array(100));},cancel(){canceled=true;}});
  await assert.rejects(prepareOcrAssets({...f,download:true,fetchImpl:async()=>new Response(stream)}),/size/);
  assert.equal(canceled,true);
});
test('HTTP failures and missing offline manifest do not count as prepared',async t=>{
  const f=await fixture(t);
  await assert.rejects(prepareOcrAssets({...f,download:true,fetchImpl:async()=>new Response('',{status:404})}),/404/);
  await prepareOcrAssets({...f,download:true,fetchImpl:async()=>new Response(f.model)});
  await rm(join(f.root,'public/ocr/manifest.json'));
  await assert.rejects(prepareOcrAssets({...f,download:false}),/manifest.*setup:ocr/);
});
