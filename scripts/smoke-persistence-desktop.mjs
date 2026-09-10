import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdirSync,readFileSync,writeFileSync,copyFileSync,existsSync,renameSync} from 'node:fs';
import {resolve,dirname,basename} from 'node:path';
import {createHash} from 'node:crypto';
const exec=promisify(execFile),pid=process.env.FOLIO_APP_PID;
if(!pid)throw new Error('Set FOLIO_APP_PID to an isolated test app.');
const artifacts=resolve('artifacts/persistence-desktop-'+Date.now());mkdirSync(artifacts,{recursive:true});
const source=resolve(artifacts,'Source.pdf'),moved=resolve(artifacts,'Moved original.pdf');
copyFileSync(resolve('examples/Welcome to Folio.pdf'),source);
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex'),sourceHash=hash(source);
const outputs=['Editable.pdf','Edited again.pdf','Flattened.pdf','Deleted.pdf'].map(name=>resolve(artifacts,name));
const browser=await chromium.connectOverCDP(process.env.FOLIO_CDP_URL||'http://127.0.0.1:9228');
const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
const errors=[];page.on('pageerror',error=>errors.push(error.message));
const canvas=page.locator('.document-page').first().locator('.page-canvas');
async function dialog(action,path){await exec('pwsh',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',pid,'-FilePath',path,'-Action',action],{timeout:60000,windowsHide:true});}
async function open(path,first=false){await page.getByRole('button',{name:first?'Open a PDF':'Open',exact:true}).click();await dialog('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true})).toHaveAttribute('aria-selected','true');await expect(canvas.locator('image')).toHaveCount(1);console.log('Opened '+basename(path));}
async function save(path,flatten=false){
 const nativeOutput=resolve(artifacts,'Folio edited.pdf');
 if(dirname(nativeOutput)!==artifacts||dirname(path)!==artifacts||existsSync(nativeOutput)||existsSync(path))throw new Error('Output must be unused inside the current test artifact directory');
 if(flatten){await page.getByRole('button',{name:'Save options',exact:true}).click();await page.getByRole('menuitem',{name:/Flatten text and ink/}).click();}
 else await page.getByRole('button',{name:'Save a copy',exact:true}).click();
 await dialog('Save',nativeOutput);await expect(page.locator('.statusbar')).toContainText(flatten?'Exported flattened text and ink':'Saved a copy',{timeout:15000});
 await expect.poll(()=>existsSync(nativeOutput)).toBe(true);renameSync(nativeOutput,path);console.log('Saved '+basename(path));
}
async function selectText(){await canvas.locator('[data-overlay] tspan').first().click();await expect(page.getByRole('textbox',{name:'Content',exact:true})).toBeVisible();}
async function selectInk(){const point=await canvas.locator('[data-overlay] polyline').first().evaluate(line=>{const local=line.points.getItem(Math.floor(line.points.numberOfItems/2)),screen=new DOMPoint(local.x,local.y).matrixTransform(line.getScreenCTM());return{x:screen.x,y:screen.y};});await page.mouse.click(point.x,point.y);await expect(page.getByRole('spinbutton',{name:'Ink width',exact:true})).toBeVisible();}
try{
 await open(source,true);
 await page.getByRole('button',{name:'Text',exact:true}).click();await canvas.click({position:{x:100,y:285}});
 await page.getByRole('textbox',{name:'Content',exact:true}).fill('Portable editable text');await page.getByRole('spinbutton',{name:'Size',exact:true}).fill('22');
 await page.getByRole('button',{name:'Draw',exact:true}).click();let box=await canvas.boundingBox();
 await page.mouse.move(box.x+60,box.y+190);await page.mouse.down();for(const[x,y]of[[90,170],[125,195],[155,175],[185,193]])await page.mouse.move(box.x+x,box.y+y,{steps:4});await page.mouse.up();
 await page.getByRole('button',{name:'Signature',exact:true}).click();await page.getByRole('button',{name:'Draw new',exact:true}).click();
 const pad=await page.getByLabel('Signature drawing area').boundingBox();await page.mouse.move(pad.x+30,pad.y+70);await page.mouse.down();for(const[x,y]of[[60,25],[80,100],[100,45],[160,80],[230,50]])await page.mouse.move(pad.x+x,pad.y+y,{steps:4});await page.mouse.up();
 await page.getByRole('button',{name:'Use signature',exact:true}).click();box=await canvas.boundingBox();await page.mouse.click(box.x+100,box.y+455);
 await expect(canvas.locator('[data-overlay]')).toHaveCount(3);
 await page.getByRole('button',{name:'Comment',exact:true}).click();await canvas.click({position:{x:230,y:250}});await page.getByRole('textbox',{name:'Comment',exact:true}).fill('Portable review note');
 await page.getByRole('button',{name:'Select',exact:true}).click();await canvas.click({position:{x:15,y:15}});
 await page.getByRole('button',{name:'Rotate',exact:true}).click();await page.getByRole('button',{name:'Duplicate',exact:true}).click();await expect(page.getByLabel('Page 2 of 4',{exact:true})).toHaveClass(/selected/);
 await page.getByRole('button',{name:'Move down',exact:true}).click();await page.getByLabel('Page 1 of 4',{exact:true}).click();await expect(canvas.locator('[data-overlay]')).toHaveCount(4);
 await save(outputs[0]);await page.screenshot({path:resolve(artifacts,'01-editable.png')});
 await page.getByRole('button',{name:'Close Source.pdf',exact:true}).click();
 // Exercise a portable PDF without access to its original path or any open source tab.
 if(dirname(source)!==artifacts||dirname(moved)!==artifacts||existsSync(moved))throw new Error('Unsafe test source rename');renameSync(source,moved);
 await open(outputs[0],true);await expect(canvas.locator('[data-overlay]')).toHaveCount(4);
 await selectText();await expect(page.getByRole('textbox',{name:'Content',exact:true})).toHaveValue('Portable editable text');await expect(page.getByRole('spinbutton',{name:'Size',exact:true})).toHaveValue('22');
 await page.getByRole('textbox',{name:'Content',exact:true}).fill('Edited after reopen');
 await selectInk();const inkWidth=page.getByRole('spinbutton',{name:'Ink width',exact:true});await inkWidth.fill('50');await inkWidth.blur();await expect(inkWidth).toHaveValue('50');
 await page.getByRole('button',{name:'Undo',exact:true}).click();await expect(inkWidth).not.toHaveValue('50');await page.getByRole('button',{name:'Redo',exact:true}).click();await expect(inkWidth).toHaveValue('50');
 await page.screenshot({path:resolve(artifacts,'02-reopened-editing.png')});await save(outputs[1]);await open(outputs[1]);
 await expect(canvas.locator('[data-overlay]')).toHaveCount(4);await selectText();await expect(page.getByRole('textbox',{name:'Content',exact:true})).toHaveValue('Edited after reopen');
 await page.getByRole('textbox',{name:'Content',exact:true}).fill('Flattened visible text');await save(outputs[2],true);await expect(page.getByLabel('Unsaved changes')).toHaveCount(1);
 await open(outputs[2]);await expect(canvas.locator('[data-overlay]')).toHaveCount(1);await expect(page.getByRole('button',{name:'Page 1: Portable review note',exact:true})).toBeVisible();
 await expect.poll(async()=>(await canvas.locator('[data-pdf-character]').allTextContents()).join('')).toContain('Flattened visible text');
 await page.screenshot({path:resolve(artifacts,'03-flattened.png')});
 await page.getByRole('tab',{name:'Edited again.pdf',exact:true}).click();await selectText();await page.getByRole('button',{name:'Delete text',exact:true}).click();
 await selectInk();await page.getByRole('button',{name:'Delete ink',exact:true}).click();await selectInk();await page.getByRole('button',{name:'Delete ink',exact:true}).click();
 await expect(canvas.locator('[data-overlay]')).toHaveCount(1);await save(outputs[3]);await open(outputs[3]);await expect(canvas.locator('[data-overlay]')).toHaveCount(1);
 // The duplicated, moved page retains its own original additions independently.
 await page.getByLabel('Page 3 of 4',{exact:true}).click();const duplicate=page.locator('.document-page').nth(2).locator('.page-canvas');await expect(duplicate.locator('[data-overlay]')).toHaveCount(4);
 expect(hash(moved)).toBe(sourceHash);expect(errors).toEqual([]);
 const binary=JSON.parse(readFileSync(resolve('artifacts/desktop-process.json'),'utf8').replace(/^\uFEFF/,'')).binary;
 const result={passed:true,binary,binarySha256:hash(binary),source:moved,sourceSha256:sourceHash,outputs,checks:['text/ink/signature editability','rotated and duplicated/moved pages','source independence','edit-resave-reopen without duplicates','ink resize undo/redo','flattened text/ink stays dirty','flatten retains review annotations','deleted additions stay deleted','duplicate edits isolated'],errors};
 await exec('pwsh',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('scripts/request-native-close.ps1'),'-AppProcessId',pid],{windowsHide:true});
 await expect.poll(async()=>{try{await exec('pwsh',['-NoProfile','-Command','if(Get-Process -Id '+pid+' -ErrorAction SilentlyContinue){exit 1}'],{windowsHide:true});return true;}catch{return false;}},{timeout:15000}).toBe(true);
 writeFileSync(resolve(artifacts,'results.json'),JSON.stringify(result,null,2));console.log(JSON.stringify({passed:true,artifacts,...result},null,2));
}catch(error){await page.screenshot({path:resolve(artifacts,'failure.png')}).catch(()=>{});writeFileSync(resolve(artifacts,'failure.txt'),String(error.stack));throw error;}finally{await browser.close();}