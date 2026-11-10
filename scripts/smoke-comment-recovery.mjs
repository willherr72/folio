// Run sequential stages with the actual previous/current executables and one isolated data directory.
import {chromium,expect} from '@playwright/test';
import {readFileSync,writeFileSync,readdirSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {createHash} from 'node:crypto';
const stage=process.argv[2],root=resolve('artifacts/comment-fidelity-downgrade'),profile=join(root,'profile');
const record=JSON.parse(readFileSync(join(profile,'process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`),page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});
const snapshot=directory=>Object.fromEntries(readdirSync(directory,{withFileTypes:true}).filter(entry=>entry.name!=='recovery.lock').flatMap(entry=>{
 const path=join(directory,entry.name);return entry.isDirectory()?Object.entries(snapshot(path)).map(([key,value])=>[entry.name+'/'+key,value]):[[entry.name,createHash('sha256').update(readFileSync(path)).digest('hex')]];
}));
async function openFixture(){
 const opening=invoke('open_pdf');
 await promisify(execFile)('pwsh',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-Action','Open','-FilePath',resolve('artifacts/comment-fidelity-desktop/Comment fidelity.pdf'),'-NativeFileName'],{windowsHide:true,timeout:60000});
 return opening;
}
function workspace(info){return {version:1,activeId:'tab',tabs:[{id:'tab',savedDigest:'old',dirty:true,zoom:100,scrollPosition:{top:0,left:0},document:{name:'Mixed migrated draft',pages:info.pages.map((p,i)=>({id:`p${i}`,sourceId:info.id,pageIndex:i,width:p.width,height:p.height,rotation:0,overlays:(p.overlays??[]).filter(o=>o.text!=='Remove this note').map(o=>o.text==='Review this paragraph'?{...o,text:'Legacy edited overlay'}:o)})),selectedPageId:'p0',selectedOverlayId:null}}]};}
try{
 await page.waitForFunction(()=>!!window.__TAURI_INTERNALS__);
 // Let the app finish its initial recovery effect before driving native commands.
 if(stage==='migrate-and-mix'||stage==='verify-current'){
  const restore=page.getByRole('button',{name:'Restore workspace',exact:true}),tab=page.getByRole('tab',{name:'Mixed migrated draft',exact:true});
  await expect(restore.or(tab).first()).toBeVisible();if(await restore.isVisible())await restore.click();
  await expect(tab).toBeVisible();
 }else await expect(page.getByRole('button',{name:'Open a PDF',exact:true})).toBeVisible();
 await new Promise(resolve=>setTimeout(resolve,1200));
 if(stage==='seed-legacy'){
  const info=await openFixture();expect(info.pages[1].overlays.filter(o=>o.type==='comment')).toHaveLength(3);await invoke('save_recovery',{workspace:workspace(info)});
 }else if(stage==='migrate-and-mix'){
  const restored=await invoke('load_recovery');expect(restored.tabs[0].document.pages[1].overlays.map(o=>o.text)).toContain('Legacy edited overlay');expect(restored.tabs[0].document.pages[1].overlays.map(o=>o.text)).not.toContain('Remove this note');
  const oldNotes=await invoke('list_native_comments',{sourceId:restored.tabs[0].document.pages[0].sourceId});expect(oldNotes.map(n=>n.text)).toEqual(['Locked reference note']);
  if(!restored.tabs[0].document.pages.some(p=>p.id==='fresh')){const fresh=await openFixture();expect(fresh.pages[1].overlays).toHaveLength(0);restored.tabs[0].document.pages.push({id:'fresh',sourceId:fresh.id,pageIndex:1,width:fresh.pages[1].width,height:fresh.pages[1].height,rotation:0,overlays:[]});}await invoke('save_recovery',{workspace:restored});
  writeFileSync(join(root,'protected-files.json'),JSON.stringify(snapshot(join(profile,'data/recovery/comment-format-3')),null,2));
 }else if(stage==='old-cleanup'){
  expect(await invoke('load_recovery')).toBeNull();await invoke('clear_recovery');const blank=await invoke('create_blank_document',{width:300,height:400});await invoke('save_recovery',{workspace:workspace(blank)});
  expect(snapshot(join(profile,'data/recovery/comment-format-3'))).toEqual(JSON.parse(readFileSync(join(root,'protected-files.json'),'utf8')));
 }else if(stage==='verify-current'){
  const restored=await invoke('load_recovery');expect(restored.tabs[0].document.pages).toHaveLength(3);const pages=restored.tabs[0].document.pages;expect(pages[1].overlays.map(o=>o.text)).toContain('Legacy edited overlay');expect(pages[1].overlays.map(o=>o.text)).not.toContain('Remove this note');
  expect((await invoke('list_native_comments',{sourceId:pages[0].sourceId})).map(n=>n.text)).toEqual(['Locked reference note']);
  expect((await invoke('list_native_comments',{sourceId:pages[2].sourceId})).map(n=>n.text)).toEqual(['Locked reference note','Review this paragraph','Color note','Remove this note']);
  await page.setViewportSize({width:900,height:600});await page.getByRole('button',{name:'Page 3: Review this paragraph',exact:true}).click();const dialog=page.getByRole('dialog',{name:'PDF comment'});await expect(dialog).toBeVisible();const box=await dialog.boundingBox();expect(box.y).toBeGreaterThanOrEqual(0);expect(box.y+box.height).toBeLessThanOrEqual(600);await page.getByRole('button',{name:'Close',exact:true}).scrollIntoViewIfNeeded();await page.screenshot({path:join(root,'minimum-window-dialog.png')});await page.getByRole('button',{name:'Close',exact:true}).click();
 }else throw new Error('Unknown recovery stage');
 writeFileSync(join(root,`${stage}.json`),JSON.stringify({passed:true,stage,executableSha256:record.sha256},null,2));console.log('PASS '+stage);
}finally{await browser.close();}
