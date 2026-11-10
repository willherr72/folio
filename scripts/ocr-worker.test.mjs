import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {runInNewContext} from 'node:vm';
const script=await readFile(new URL('./ocr-worker.js',import.meta.url),'utf8');
function worker() {
  const loads=[],requests=[];
  const self={location:{href:'http://tauri.localhost/ocr/worker.js'},
    importScripts:(...urls)=>loads.push(...urls),
    fetch:(...args)=>{requests.push(args);return Promise.resolve('ok');}};
  runInNewContext(script,{self,URL,Request,TypeError});
  return {self,loads,requests};
}
test('worker imports local pinned script and accepts only local asset URLs',async()=>{
  const w=worker();assert.deepEqual(w.loads,['http://tauri.localhost/ocr/upstream/worker.min.js']);
  await w.self.fetch('http://tauri.localhost/ocr/models/eng.traineddata');
  w.self.importScripts('http://tauri.localhost/ocr/core/tesseract-core-lstm.wasm.js');
  assert.equal(w.requests.length,1);assert.equal(w.loads.length,2);
});
test('worker blocks remote, traversal, query leakage and mutation requests',async()=>{
  const w=worker();
  for(const url of ['https://cdn.example/ocr/core.js','http://tauri.localhost/private',
    '../private','./models/../../private','./models/eng.traineddata?text=secret','./models/eng.traineddata#secret']){
    assert.throws(()=>w.self.importScripts(url),/local OCR/);
    await assert.rejects(Promise.resolve().then(()=>w.self.fetch(url)),/local OCR/);
  }
  await assert.rejects(Promise.resolve().then(()=>w.self.fetch('./models/eng.traineddata',{method:'POST',body:'scan'})),/local OCR/);
  assert.equal(w.requests.length,0);assert.equal(w.loads.length,1);
});
