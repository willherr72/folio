// Uses only the isolated copied Windows executable recorded by start-release-test.ps1.
import {chromium,expect} from '@playwright/test';import {execFile} from 'node:child_process';import {promisify} from 'node:util';
import {readFileSync,writeFileSync,existsSync,renameSync} from 'node:fs';import {resolve,basename} from 'node:path';import {createHash} from 'node:crypto';
const exec=promisify(execFile),directory=resolve(process.env.FOLIO_PARAGRAPH_ACCEPTANCE_ROOT ?? 'artifacts/paragraph-position-desktop'),record=JSON.parse(readFileSync(resolve(directory,'profile/process.json'),'utf8').replace(/^\uFEFF/,''));
const browser=await chromium.connectOverCDP(`http://127.0.0.1:${record.port ?? 9441}`),page=browser.contexts()[0].pages().find(page=>!page.url().startsWith('devtools:'));page.setDefaultTimeout(20000);
const errors=[];page.on('pageerror',error=>errors.push(error.message));const button=name=>page.getByRole('button',{name,exact:true}),field=name=>page.getByRole('spinbutton',{name,exact:true});
async function dialog(action,path){await exec('pwsh',['-NoProfile','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-FilePath',path,'-Action',action],{windowsHide:true,timeout:60000});}
async function open(path){await page.getByRole('button',{name:/^(Open|Open a PDF)$/}).click();await dialog('Open',path);await expect(page.getByRole('tab',{name:basename(path),exact:true}).last()).toHaveAttribute('aria-selected','true');await expect(page.locator('.document-page image').first()).toBeVisible();}
async function save(name){const target=resolve(directory,name),native=resolve(directory,'Folio edited.pdf');if(existsSync(target)||existsSync(native))throw Error('Expected unused generated output');await button('Save a copy').click();await dialog('Save',native);await expect(page.locator('.statusbar')).toContainText('Saved a copy');await expect.poll(()=>existsSync(native)).toBe(true);renameSync(native,target);return target;}
async function textContains(text){await expect.poll(()=>page.locator('.document-page .pdf-text-layer').first().textContent()).toContain(text);}
async function point(x,y){return page.locator('.paragraph-page-targets').evaluate((svg,p)=>{const point=svg.createSVGPoint();point.x=p.x;point.y=p.y;const result=point.matrixTransform(svg.getScreenCTM());return{x:result.x,y:result.y};},{x,y});}
try{
 const source=resolve(directory,'Original paragraph.pdf'),hash=createHash('sha256').update(readFileSync(source)).digest('hex');
 await expect(page.getByRole('button',{name:/^(Open|Open a PDF)$/})).toBeEnabled();await open(source);
 await button('Comment').click();await page.locator('.document-page .page-canvas').first().click({position:{x:75,y:300}});await page.getByRole('textbox',{name:'Comment',exact:true}).fill('Keep position note');await button('Select').click();await save('Before move.pdf');
 await button('Rotate').click();await button('Edit text').click();await button('Edit text: Original words.').click();await button('Edit paragraph…').click();
 if(process.env.FOLIO_EXPECT_PARAGRAPH_SUGGESTION==='1')await expect(page.getByRole('textbox',{name:'Paragraph text',exact:true})).toHaveValue('Original words.');
 else await button('Check selection').click();
 const old={x:Number(await field('Left (pt)').inputValue()),y:Number(await field('Top (pt)').inputValue()),width:Number(await field('Area width (pt)').inputValue()),height:Number(await field('Area height (pt)').inputValue())};
 await field('Area width (pt)').fill('150');await field('Area height (pt)').fill('45');await field('Left (pt)').fill('180');await field('Top (pt)').fill('200');await page.getByRole('textbox',{name:'Paragraph text',exact:true}).fill('Positioned paragraph.');
 await button('Preview paragraph').click();await expect(page.getByAltText('Paragraph PDF preview')).toBeVisible();await button('Original').click();
 await page.locator('.paragraph-body').evaluate(node=>node.scrollTop=0);const a=await point(190,210),b=await point(200,220);
 await page.mouse.move(a.x,a.y);await page.mouse.down();await expect(button('Apply paragraph')).toBeDisabled();await page.mouse.move(b.x,b.y,{steps:5});await page.mouse.up();
 await expect(field('Left (pt)')).toHaveValue('190');await expect(field('Top (pt)')).toHaveValue('210');await expect(button('Apply paragraph')).toBeDisabled();
 await button('Preview paragraph').click();await expect(page.getByAltText('Paragraph PDF preview')).toBeVisible();await page.screenshot({path:resolve(directory,'01-preview.png')});await button('Apply paragraph').click();await expect(page.getByRole('dialog')).toHaveCount(0);await textContains('Positioned paragraph.');
 await button('Undo').click();await textContains('Original words.');await button('Redo').click();await textContains('Positioned paragraph.');const saved=await save('Positioned paragraph.pdf');
 await open(saved);await button('Find').click();await page.getByRole('searchbox').fill('Positioned');await expect(page.locator('.search-count')).toContainText('1 of 1');await button('Close search').click();await textContains('Neighbor stays.');await page.screenshot({path:resolve(directory,'02-reopened.png')});
 expect(errors).toEqual([]);expect(createHash('sha256').update(readFileSync(source)).digest('hex')).toBe(hash);
 writeFileSync(resolve(directory,'results.json'),JSON.stringify({passed:true,checks:['numeric position then drag invalidates preview','native preview and Apply on rotated workspace','one-step paragraph undo/redo','save/reopen Find and neighbor retained'],old,new:{x:190,y:210,width:150,height:45},sourceHash:hash,exe:record,errors},null,2));console.log('PASS Windows paragraph positioning acceptance');
}catch(error){writeFileSync(resolve(directory,'failure.json'),JSON.stringify({error:String(error.stack),errors},null,2));await page.screenshot({path:resolve(directory,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
