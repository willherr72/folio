import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {copyFileSync,mkdirSync,readFileSync,readdirSync,renameSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
const exec=promisify(execFile);
const dir=resolve('artifacts/recovery-desktop-'+Date.now());mkdirSync(dir,{recursive:true});
const data=resolve(dir,'data'),binary=process.env.FOLIO_TEST_BINARY;
if(!binary)throw Error('Set FOLIO_TEST_BINARY to the isolated test executable.');
const env={...process.env,FOLIO_DATA_DIR:data};
const source=resolve(dir,'Recovery source.pdf'),second=resolve(dir,'Second source.pdf');
copyFileSync(resolve('examples/Welcome to Folio.pdf'),source);copyFileSync(source,second);
let pid,browser,page;const errors=[];
async function ps(args){return exec('pwsh',['-NoProfile','-ExecutionPolicy','Bypass',...args],{env,windowsHide:true,timeout:60000});}
async function start(){
 const result=await ps(['-File',resolve('scripts/start-desktop-test.ps1'),'-Binary',binary,'-Port','9238']);
 const state=JSON.parse(result.stdout);pid=state.pid;
 browser=await chromium.connectOverCDP('http://127.0.0.1:9238');
 page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
 page.on('pageerror',error=>errors.push(error.message));
}
async function open(path,first=false){
 await page.getByRole('button',{name:first?'Open a PDF':'Open',exact:true}).click();
 await ps(['-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(pid),'-FilePath',path,'-Action','Open']);
}
function latest(){
 const folder=resolve(data,'recovery');const names=readdirSync(folder).filter(n=>/^checkpoint-.*\.json$/.test(n)).sort();
 return JSON.parse(readFileSync(resolve(folder,names.at(-1)),'utf8')).workspace;
}
async function close(){
 const hadTabs=await page.getByRole('tab').count();
 await ps(['-File',resolve('scripts/request-native-close.ps1'),'-AppProcessId',String(pid)]);
 if(hadTabs){
  await page.getByRole('button',{name:'Discard and close',exact:true}).click();
 }
 await expect.poll(async()=>{try{await ps(['-Command','if(Get-Process -Id '+pid+' -ErrorAction SilentlyContinue){exit 1}']);return true;}catch{return false;}},{timeout:15000}).toBe(true);
 try{await browser.close();}catch{}
}
try{
 await start();await open(source,true);await expect(page.getByRole('tab',{name:'Recovery source.pdf'})).toBeVisible();
 await page.getByRole('button',{name:'Text',exact:true}).click();
 await page.locator('.page-canvas').first().click({position:{x:100,y:285}});
 await page.getByRole('textbox',{name:'Content'}).fill('Recovery marker');
 await page.getByRole('button',{name:'Select',exact:true}).click();
 await page.getByRole('button',{name:'Zoom in',exact:true}).click();await page.getByRole('button',{name:'Zoom in',exact:true}).click();
 await page.getByRole('button',{name:'Rotate',exact:true}).click();
 await page.getByRole('button',{name:'Duplicate',exact:true}).click();
 await open(second);await page.getByRole('button',{name:'Rotate',exact:true}).click();
 await expect.poll(()=>{try{const snap=latest();return snap.tabs.length===2&&snap.tabs[0].zoom===110&&snap.tabs[0].document.pages.length===4&&snap.tabs.every(t=>t.dirty);}catch{return false;}},{timeout:15000}).toBe(true);
 const before=latest();writeFileSync(resolve(dir,'checkpoint-before.json'),JSON.stringify(before,null,2));
 await page.screenshot({path:resolve(dir,'before-crash.png')});
 const escaped=resolve(binary).replaceAll("'","''");
 await ps(['-Command',"$p=Get-Process -Id "+pid+" -ErrorAction Stop;if($p.Path -ne '"+escaped+"'){throw 'Unexpected process path'};Stop-Process -Id "+pid+" -Force"]);
 try{await browser.close();}catch{}
 renameSync(source,source+'.moved');renameSync(second,second+'.moved');
 await start();
 await expect(page.getByRole('dialog',{name:'Restore your workspace?'})).toBeVisible();
 await page.getByRole('button',{name:'Restore workspace',exact:true}).click();
 await expect(page.getByRole('tab',{name:'Second source.pdf'})).toHaveAttribute('aria-selected','true');
 await page.getByRole('tab',{name:'Recovery source.pdf'}).click();
 await expect(page.getByRole('button',{name:'110%',exact:true})).toBeVisible();
 await expect(page.locator('.document-page')).toHaveCount(4);
 await expect(page.locator('.page-canvas [data-overlay] tspan').first()).toHaveText('Recovery marker');
 await page.keyboard.press('Control+f');await page.getByRole('searchbox',{name:'Find in document'}).fill('Recovery marker');
 await expect(page.locator('.search-count')).toHaveText('1 of 2');await page.keyboard.press('Enter');await expect(page.locator('.search-count')).toHaveText('2 of 2');
 await expect(page.locator('.search-highlight.active').first()).toBeVisible();
 await page.keyboard.press('Escape');
 await page.screenshot({path:resolve(dir,'restored.png')});
 await page.keyboard.press('Control+p');await expect(page.getByRole('dialog',{name:'Print document'})).toBeVisible();
 await page.screenshot({path:resolve(dir,'print-options.png')});
 await page.getByRole('button',{name:'Choose printer…'}).click();
 await ps(['-File',resolve('scripts/cancel-native-print.ps1'),'-AppProcessId',String(pid)]);
 await expect(page.getByText('Printing cancelled',{exact:true})).toBeVisible();
 await expect.poll(()=>{try{return latest().tabs[0].document.pages[0].sourceId!==before.tabs[0].document.pages[0].sourceId;}catch{return false;}},{timeout:15000}).toBe(true);
 await close();await start();
 await expect(page.getByRole('button',{name:'Open a PDF',exact:true})).toBeEnabled();
 await expect(page.getByRole('dialog',{name:'Restore your workspace?'})).toHaveCount(0);
 await close();
 expect(errors).toEqual([]);
 const result={passed:true,crashRecovery:true,movedOriginals:true,dirtyTabs:2,duplicatedAnnotations:true,zoom:110,searchDuplicates:true,normalCloseClearsRecovery:true,nativePrintDialogCancellation:true,errors,artifacts:dir};
 writeFileSync(resolve(dir,'results.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
}catch(error){if(page)await page.screenshot({path:resolve(dir,'failure.png')}).catch(()=>{});console.error('Test artifacts:',dir);throw error;}
