// Repository Windows/WebView workflow: native dialogs and real IPC, no mocked engine.
import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {copyFileSync,existsSync,mkdirSync,readFileSync,writeFileSync,readdirSync,statSync} from 'node:fs';
import {resolve,basename} from 'node:path';
import {createHash} from 'node:crypto';
const run=resolve(process.env.FOLIO_RELEASE_RUN),exec=promisify(execFile);
const state=JSON.parse(readFileSync(resolve(run,'process.json'),'utf8').replace(/^\uFEFF/,''));
const corpus=resolve(run,'workflow');mkdirSync(corpus);
const fixture=resolve('artifacts/page-occurrence-native-20261024/corpus-final');
const source=resolve(corpus,'original.pdf'),incoming=resolve(corpus,'incoming.pdf');
copyFileSync(resolve(fixture,'r0-u1-s0-source.pdf'),source);copyFileSync(resolve(fixture,'r0-u1-s0-incoming.pdf'),incoming);
const hash=p=>createHash('sha256').update(readFileSync(p)).digest('hex');
const hashes={source:hash(source),incoming:hash(incoming)},errors=[];
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${state.port}`);
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));page.setDefaultTimeout(25000);
page.on('pageerror',e=>errors.push(String(e)));
async function dialog(title,path,action){await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-release-dialog.ps1'),'-AppProcessId',String(state.pid),'-FilePath',path,'-Title',title,'-Action',action],{windowsHide:true,timeout:25000});}
async function save(name){const path=resolve(corpus,name);await page.getByRole('button',{name:'Save a copy',exact:true}).click();await dialog('Save a PDF copy',path,'Save');await expect.poll(()=>existsSync(path)).toBe(true);await expect(page.getByRole('button',{name:'Save a copy',exact:true})).toBeEnabled();return path;}
async function choose(){await page.getByRole('button',{name:'Choose PDF…',exact:true}).click();await dialog('Open a PDF',incoming,'Open');await expect(page.getByText('incoming.pdf',{exact:true})).toBeVisible();}
async function preview(){await page.getByRole('button',{name:'Preview replacement',exact:true}).click();await expect(page.getByRole('button',{name:'Apply replacement',exact:true})).toBeEnabled();}
try {
 await page.getByRole('button',{name:'Open a PDF',exact:true}).click();await dialog('Open a PDF',source,'Open');
 await expect(page.getByRole('tab',{name:basename(source),exact:true})).toBeVisible();
 await expect(page.locator('.page-canvas').first().locator('image')).toHaveCount(1);
 await page.getByRole('button',{name:'Rotate',exact:true}).click();
 await page.getByRole('button',{name:'Crop page…',exact:true}).click();
 await page.getByLabel('Left margin (pt)').fill('12');await page.getByLabel('Top margin (pt)').fill('8');
 await page.getByRole('button',{name:'Preview crop',exact:true}).click();await expect(page.getByRole('button',{name:'Apply crop',exact:true})).toBeEnabled();await page.getByRole('button',{name:'Apply crop',exact:true}).click();
 await page.getByRole('button',{name:'Duplicate',exact:true}).click();await save('baseline.pdf');
 await page.getByRole('button',{name:'Page 2 of 3',exact:true}).click();await expect(page.getByRole('button',{name:'Go to page',exact:true})).toHaveText('Page 2 of 3');
 await page.getByRole('button',{name:'Replace page content…',exact:true}).click();await choose();await preview();
 await page.getByRole('button',{name:'Cancel',exact:true}).click();
 await expect(page.getByRole('button',{name:'Replace page content…',exact:true})).toBeFocused();
 await page.getByRole('button',{name:'Page 2 of 3',exact:true}).click();await expect(page.getByRole('button',{name:'Go to page',exact:true})).toHaveText('Page 2 of 3');
 await page.getByRole('button',{name:'Replace page content…',exact:true}).click();await choose();
 await page.getByLabel('Incoming page',{exact:true}).fill('2');await preview();
 await page.getByLabel('Incoming page',{exact:true}).fill('1');await expect(page.getByRole('button',{name:'Apply replacement',exact:true})).toBeDisabled();await preview();
 await page.screenshot({path:resolve(run,'replacement-preview.png')});
 await page.setViewportSize({width:600,height:650});
 await page.getByRole('button',{name:'Apply replacement',exact:true}).scrollIntoViewIfNeeded();
 const layout=await page.getByRole('dialog').evaluate(element=>({width:element.clientWidth,scrollWidth:element.scrollWidth,viewport:window.innerWidth}));
 expect(layout.scrollWidth).toBeLessThanOrEqual(layout.width+1);expect(layout.width).toBeLessThan(layout.viewport);
 await page.screenshot({path:resolve(run,'replacement-narrow.png')});await page.setViewportSize({width:1280,height:820});
 await page.getByRole('button',{name:'Apply replacement',exact:true}).click();await expect(page.getByRole('dialog')).toHaveCount(0);
 await page.getByRole('button',{name:'Undo',exact:true}).click();await save('undone.pdf');
 await page.getByRole('button',{name:'Redo',exact:true}).click();await save('replaced.pdf');
 await page.getByRole('button',{name:'Open',exact:true}).click();await dialog('Open a PDF',resolve(corpus,'replaced.pdf'),'Open');await expect(page.getByRole('tab',{name:'replaced.pdf',exact:true,selected:true})).toBeVisible();
 await page.getByRole('button',{name:'Find',exact:true}).click();await page.getByRole('searchbox').fill('New content');await expect(page.locator('.search-count')).toHaveText('1 of 1');await page.keyboard.press('Escape');
 await page.screenshot({path:resolve(run,'replacement-reopened.png')});
 const checkpointAfter=Date.now();await page.getByRole('tab',{name:'original.pdf',exact:true}).click();
 const recovery=resolve(run,'data/recovery/comment-format-3');
 await expect.poll(()=>{try{return readdirSync(recovery).filter(name=>/^checkpoint-.*\.json$/.test(name)).some(name=>statSync(resolve(recovery,name)).mtimeMs>=checkpointAfter);}catch{return false;}},{timeout:15000}).toBe(true);
 expect(hash(source)).toBe(hashes.source);expect(hash(incoming)).toBe(hashes.incoming);expect(errors).toEqual([]);
 writeFileSync(resolve(run,'results.json'),JSON.stringify({passed:true,hashes,errors,exe:state,checks:['native file dialogs','crop and rotation retained','selected duplicate','Cancel without applying','page change invalidates preview','preview before Apply','Undo and Redo','save and reopen search'],limits:['Native source cleanup is covered by focused tests; the frozen Tauri invoke method does not allow this harness to observe close IPC.']},null,2),{flag:'wx'});
 console.log('PASS actual Windows replacement, undo, save and reopen');
}catch(error){await page.screenshot({path:resolve(run,'failure.png')}).catch(()=>{});throw error;}
finally{await browser.close();}
