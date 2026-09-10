import { chromium, expect } from '@playwright/test';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdirSync, readFileSync, writeFileSync, copyFileSync, existsSync, renameSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { createHash } from 'node:crypto';
const exec = promisify(execFile);
const pid=process.env.FOLIO_APP_PID;
if(!pid)throw new Error('Set FOLIO_APP_PID to the isolated test app.');
const artifacts=resolve('artifacts/review-desktop-'+Date.now());mkdirSync(artifacts,{recursive:true});
const signatureName='Desktop review signature '+Date.now();
const source=resolve(artifacts,'Review source.pdf'), output=resolve(artifacts,'Reviewed.pdf'), deleted=resolve(artifacts,'Notes removed.pdf');
copyFileSync(resolve('examples/Welcome to Folio.pdf'),source);
const sourceHash=createHash('sha256').update(readFileSync(source)).digest('hex');
const browser=await chromium.connectOverCDP(process.env.FOLIO_CDP_URL||'http://127.0.0.1:9228');
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
const errors=[];page.on('pageerror',e=>errors.push(e.message));
async function dialog(action,path){await exec('pwsh',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',pid,'-FilePath',path,'-Action',action],{timeout:60000,windowsHide:true});}
async function open(path,first=false){await page.getByRole('button',{name:first?'Open a PDF':'Open',exact:true}).click();await dialog('Open',path);await expect(page.getByRole('tab',{name:path.split(/[\\/]/).at(-1),exact:true})).toHaveAttribute('aria-selected','true');}
async function save(path){
 const nativeOutput=resolve(artifacts,'Folio edited.pdf');
 if(dirname(nativeOutput)!==artifacts || dirname(resolve(path))!==artifacts || existsSync(nativeOutput) || existsSync(path))throw new Error('Expected unused test output paths in the current artifacts directory');
 await page.getByRole('button',{name:'Save a copy',exact:true}).click();await dialog('Save',nativeOutput);
 await expect(page.locator('.statusbar')).toContainText('Saved a copy',{timeout:15000});
 await expect.poll(()=>existsSync(nativeOutput)).toBe(true);
 // Keep native filename entry out of this test: Windows automation cannot reliably
 // commit custom names on all desktops. Rename only this generated test output.
 renameSync(nativeOutput,path);
}

const canvas=page.locator('.document-page').first().locator('.page-canvas');
async function selectPhrase(phrase){
 const glyphs=canvas.locator('[data-pdf-character]');
 await expect.poll(async()=>(await glyphs.allTextContents()).join('')).toContain(phrase);
 const chars=await glyphs.allTextContents();let offset=chars.join('').indexOf(phrase),cursor=0,start=-1,end=-1;
 for(let i=0;i<chars.length;i++){if(cursor===offset)start=i;cursor+=chars[i].length;if(cursor===offset+phrase.length){end=i;break;}}
 const a=await glyphs.nth(start).boundingBox(),b=await glyphs.nth(end).boundingBox();
 await page.mouse.move(a.x+a.width*.25,a.y+a.height/2);await page.mouse.down();await page.mouse.move(b.x+b.width*.75,b.y+b.height/2,{steps:15});await page.mouse.up();
}
try{
 await open(source,true);await expect(canvas.locator('image')).toHaveCount(1);
 await page.getByRole('button',{name:'Highlight',exact:true}).click();await selectPhrase('Make it yours.');
 await expect(page.getByRole('button',{name:'Page 1: Highlight',exact:true})).toBeVisible();
 await page.getByRole('button',{name:'Page 1: Highlight',exact:true}).click();
 await page.getByRole('textbox',{name:'Highlight note',exact:true}).fill('Read before signing');
 await page.getByRole('button',{name:'Comment',exact:true}).click();await canvas.click({position:{x:210,y:250}});
 await expect(page.getByRole('textbox',{name:'Comment',exact:true})).toBeFocused();
 await page.getByRole('textbox',{name:'Comment',exact:true}).fill('Payment terms checked ✓');
 await page.getByRole('button',{name:'Signature',exact:true}).click();
 await page.getByRole('button',{name:'Draw new',exact:true}).click();
 const pad=await page.getByLabel('Signature drawing area').boundingBox();
 await page.mouse.move(pad.x+30,pad.y+70);await page.mouse.down();
 for(const [x,y] of [[60,25],[80,100],[100,45],[160,80],[230,50]])await page.mouse.move(pad.x+x,pad.y+y,{steps:4});
 await page.mouse.up();await page.getByLabel('Signature name',{exact:true}).fill(signatureName);
 await page.getByRole('button',{name:'Save to library',exact:true}).click();
 await page.getByRole('button',{name:'Use signature',exact:true}).click();
 const box=await canvas.boundingBox();await page.mouse.move(box.x+70,box.y+420);
 await expect(canvas.locator('.signature-preview')).toBeVisible();await page.mouse.click(box.x+70,box.y+420);
 const inkWidth=page.getByRole('spinbutton',{name:'Ink width',exact:true});
 await inkWidth.fill('160');await inkWidth.blur();await expect(inkWidth).toHaveValue('160');
 await page.getByRole('button',{name:'Undo',exact:true}).click();await expect(inkWidth).not.toHaveValue('160');
 await page.getByRole('button',{name:'Redo',exact:true}).click();await expect(inkWidth).toHaveValue('160');
 await page.getByRole('button',{name:'Signature',exact:true}).click();
 await page.getByRole('button',{name:'Select signature '+signatureName,exact:true}).click();
 await page.getByRole('button',{name:'Use signature',exact:true}).click();await page.mouse.move(box.x+100,box.y+450);
 await expect(canvas.locator('.signature-preview')).toBeVisible();await page.keyboard.press('Escape');await expect(canvas.locator('.signature-preview')).toHaveCount(0);
 await save(output);await page.screenshot({path:resolve(artifacts,'01-reviewed.png')});
 await open(output);
 await expect(page.getByRole('button',{name:'Page 1: Payment terms checked ✓',exact:true})).toBeVisible();
 await expect(page.getByRole('button',{name:'Page 1: Read before signing',exact:true})).toBeVisible();
 await expect(page.locator('.review-entry')).toHaveCount(2);
 await page.getByRole('button',{name:'Page 1: Payment terms checked ✓',exact:true}).click();
 await expect(page.getByRole('textbox',{name:'Comment',exact:true})).toHaveValue('Payment terms checked ✓');
 await page.screenshot({path:resolve(artifacts,'03-reopened-review.png')});
 await page.getByRole('button',{name:'Delete comment',exact:true}).click();
 await page.getByRole('button',{name:'Page 1: Read before signing',exact:true}).click();
 await page.getByRole('button',{name:'Delete highlight',exact:true}).click();await save(deleted);await open(deleted);
 await expect(page.locator('.review-entry')).toHaveCount(0);
 await page.getByRole('tab',{name:'Review source.pdf',exact:true}).click();await expect(page.locator('.review-entry')).toHaveCount(2);
 await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByLabel('Theme',{exact:true}).selectOption('dark');await page.getByRole('button',{name:'Done',exact:true}).click();
 await page.getByRole('button',{name:'Signature',exact:true}).click();await page.screenshot({path:resolve(artifacts,'02-library-dark.png')});await page.getByRole('button',{name:'Cancel',exact:true}).click();
 expect(createHash('sha256').update(readFileSync(source)).digest('hex')).toBe(sourceHash);expect(errors).toEqual([]);
 const binary=JSON.parse(readFileSync(resolve('artifacts/desktop-process.json'),'utf8').replace(/^\uFEFF/,'' )).binary;
 const result={passed:true,source,output,deleted,binary,signatureName,sha256:createHash('sha256').update(readFileSync(binary)).digest('hex'),errors};
 await exec('pwsh',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('scripts/request-native-close.ps1'),'-AppProcessId',pid],{windowsHide:true});
 await expect.poll(async()=>{try{await exec('pwsh',['-NoProfile','-Command','if(Get-Process -Id '+pid+' -ErrorAction SilentlyContinue){exit 1}'],{windowsHide:true});return true;}catch{return false;}},{timeout:15000}).toBe(true);
 writeFileSync(resolve(artifacts,'results.json'),JSON.stringify(result,null,2));
 console.log(JSON.stringify({passed:true,artifacts,output,deleted}));
}catch(error){await page.screenshot({path:resolve(artifacts,'failure.png')}).catch(()=>{});writeFileSync(resolve(artifacts,'failure.txt'),String(error.stack));throw error;}
finally{await browser.close();}