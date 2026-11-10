import {chromium,expect} from '@playwright/test';
import {build} from 'esbuild';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';

const root=resolve(process.argv[2]);await mkdir(root,{recursive:false});
const fixtures=JSON.parse(await readFile(process.argv[3],'utf8'));
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const bundle=await build({entryPoints:['tests/fixtures/page-raster.tsx'],bundle:true,write:false,format:'iife',jsx:'automatic',define:{'process.env.NODE_ENV':'"production"'}});
await writeFile(join(root,'fixture.js'),bundle.outputFiles[0].contents);
const browser=await chromium.launch({headless:true}),page=await browser.newPage({viewport:{width:900,height:800}}),errors=[],cases=[],raw=[];
page.on('pageerror',error=>errors.push(error.message));
const frames=()=>page.evaluate(()=>new Promise(done=>requestAnimationFrame(()=>requestAnimationFrame(done))));
try{
 await page.setContent('<!doctype html><html><head><style>body{margin:0}svg{overflow:hidden}</style></head><body><div id="root"></div></body></html>');
 await page.addScriptTag({content:bundle.outputFiles[0].text});await expect(page.locator('canvas')).toHaveCount(1);
 const canvas=await page.locator('canvas').elementHandle();
 for(const fixture of fixtures){
  const bytes=await readFile(fixture.path);expect(sha(bytes)).toBe(fixture.sha256);
  const url=await page.evaluate(base64=>URL.createObjectURL(new Blob([Uint8Array.from(atob(base64),c=>c.charCodeAt(0))],{type:'image/png'})),bytes.toString('base64'));
  await page.evaluate(url=>window.rasterHarness.show({url,rotation:0,zoom:1,crop:false,inverse:false}),url);
  await expect.poll(()=>page.evaluate(url=>window.rasterHarness.status?.url===url&&window.rasterHarness.status?.phase==='ready'&&window.rasterHarness.status?.mode==='canvas',url)).toBe(true);
  const data=await page.locator('canvas').evaluate(canvas=>canvas.toDataURL());
  const rawPath=join(root,`${fixture.name}-raw.png`);await writeFile(rawPath,Buffer.from(data.split(',')[1],'base64'));
  raw.push({fixture:fixture.name,source:fixture.path,sourceSha256:fixture.sha256,path:rawPath,sha256:sha(await readFile(rawPath)),opaque:fixture.opaque});
  // Verify reference resource readiness, separately from canvas readiness.
  await page.evaluate(url=>new Promise((resolve,reject)=>{const image=new Image();image.onload=()=>resolve(true);image.onerror=()=>reject(Error('Reference failed'));image.src=url;}),url);
  for(const rotation of [0,90,180,270])for(const [zoom,crop,inverse] of [[1,false,false],[1.2865,false,false],[.85,true,false],[1.1,true,true]]){
   const name=`${fixture.name}-${rotation}-${zoom}-${crop}-${inverse}`;
   await page.evaluate(settings=>window.rasterHarness.show(settings),{url,rotation,zoom,crop,inverse});await frames();
   await expect(page.locator('canvas')).toHaveCount(1);expect(await canvas.evaluate(node=>node===document.querySelector('canvas'))).toBe(true);
   await page.getByRole('textbox',{name:'Draft'}).focus();
   await page.evaluate(()=>window.rasterHarness.present(false));await frames();
   const candidate=join(root,`${name}-candidate.png`);await page.locator('#page').screenshot({path:candidate});
   await page.evaluate(()=>window.rasterHarness.present(true));await frames();
   const reference=join(root,`${name}-reference.png`);await page.locator('#page').screenshot({path:reference});
   await expect(page.getByRole('textbox',{name:'Draft'})).toBeFocused();
   cases.push({name,candidate,reference,candidateSha256:sha(await readFile(candidate)),referenceSha256:sha(await readFile(reference))});
  }
  await page.evaluate(url=>window.rasterHarness.show({url,rotation:0,zoom:1,crop:false,inverse:false}),url);await frames();
  await page.evaluate(()=>window.rasterHarness.present(false));await frames();
  const count=await page.evaluate(()=>window.rasterHarness.clicks);await page.locator('#page').click({position:{x:8,y:8}});
  expect(await page.evaluate(()=>window.rasterHarness.clicks)).toBe(count+1);
  expect(await page.evaluate(()=>{const b=document.querySelector('#overlay').getBoundingClientRect();return document.elementFromPoint(b.x+5,b.y+5)?.id;})).toBe('overlay');
  await page.getByRole('textbox',{name:'Draft'}).focus();await page.keyboard.press('Tab');await expect(page.locator('#after')).toBeFocused();await page.keyboard.press('Tab');await expect(page.locator('#overlay')).toBeFocused();
  // Caller, not component, owns release. Removal must become empty before revoke.
  await page.evaluate(()=>window.rasterHarness.show({rotation:0,zoom:1,crop:false,inverse:false}));
  await expect.poll(()=>page.evaluate(()=>window.rasterHarness.status?.phase)).toBe('empty');
  await page.evaluate(url=>URL.revokeObjectURL(url),url);
 }
 expect(errors).toEqual([]);
 await writeFile(join(root,'result.json'),JSON.stringify({passed:true,scope:'standalone Chromium component; display comparisons are diagnostic',browser:browser.version(),raw,cases,errors},null,2));
 console.log(JSON.stringify({passed:true,raw:raw.length,displayPairs:cases.length,browser:browser.version()}));
}catch(error){await writeFile(join(root,'failure.json'),JSON.stringify({error:String(error.stack),raw,cases,errors},null,2));throw error;}finally{await browser.close();}
