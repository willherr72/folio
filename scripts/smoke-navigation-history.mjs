// Synthetic real-App browser acceptance; does not invoke native PDF writes.
import {chromium,expect} from '@playwright/test';
import {mkdirSync,writeFileSync,readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {createHash} from 'node:crypto';
const desktop=process.env.FOLIO_NAVIGATION_DESKTOP==='1',exec=promisify(execFile);
const directory=resolve(process.env.FOLIO_NAVIGATION_ACCEPTANCE_ROOT ?? (desktop?'artifacts/navigation-history-desktop':'artifacts/navigation-history'));mkdirSync(directory,{recursive:true});
const record=desktop?JSON.parse(readFileSync(resolve(directory,'profile/process.json'),'utf8').replace(/^\uFEFF/,'')):null;
const hashes=desktop?Object.fromEntries(['Navigation fixture.pdf','Other fixture.pdf','Short fixture.pdf'].map(name=>[name,createHash('sha256').update(readFileSync(resolve(directory,name))).digest('hex')])):{};
const browser=desktop?await chromium.connectOverCDP(`http://127.0.0.1:${record.port}`):await chromium.launch({headless:true});
const page=desktop?browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:')):await browser.newPage(),errors=[],checks=[];
await page.setViewportSize({width:900,height:600});
page.setDefaultTimeout(10000);page.setDefaultNavigationTimeout(15000);
page.on('pageerror',e=>errors.push(e.message));
const button=name=>page.getByRole('button',{name,exact:true});
const reviewText=desktop?'Navigation acceptance note':'Review this paragraph';
const back=()=>button('Back to previous view').click(),forward=()=>button('Forward to next view').click();
const view=()=>page.evaluate(()=>{
 const host=document.querySelector('.document-viewport'),v=host.getBoundingClientRect(),x=v.left+v.width/2,y=v.top+v.height/2,zoom=parseFloat(document.querySelector('.zoom-value').textContent);
 const slots=[...host.querySelectorAll('.document-page')].map(slot=>({id:slot.dataset.pageId,rect:slot.getBoundingClientRect()}));slots.sort((a,b)=>Math.max(a.rect.top-y,y-a.rect.bottom,0)-Math.max(b.rect.top-y,y-b.rect.bottom,0));
 const nearest=slots[0];return {id:nearest.id,zoom,x:(x-nearest.rect.left)*100/zoom,y:(y-nearest.rect.top)*100/zoom,scrollTop:host.scrollTop,scrollLeft:host.scrollLeft};
});
async function expectView(expected){try{await expect.poll(async()=>{const actual=await view();return actual.id===expected.id&&actual.zoom===expected.zoom&&Math.abs(actual.x-expected.x)<1.5&&Math.abs(actual.y-expected.y)<1.5;}).toBe(true);}catch(error){console.log(JSON.stringify({expected,actual:await view(),checks},null,2));throw error;}}
async function jump(number){await button('Go to page').click();await page.getByRole('spinbutton',{name:'Page number'}).fill(String(number));await button('Go').click();await expect(button('Go to page')).toHaveText(`Page ${number} of 4`);}
async function openDialog(name){await exec('pwsh',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',String(record.pid),'-Action','Open','-FilePath',resolve(directory,name),'-NativeFileName'],{windowsHide:true,timeout:60000});}
try{
 if(desktop){await button('Open a PDF').click();await openDialog('Navigation fixture.pdf');}
 else await page.goto('http://127.0.0.1:1437/tests/fixtures/navigation-history.html');
 await expect(button('Go to page')).toHaveText('Page 1 of 4');
 await expect(button('Back to previous view')).toBeDisabled();
 await page.locator('.document-viewport').evaluate(host=>{host.scrollTop=350;host.scrollLeft=40;});
 await expect.poll(()=>page.locator('.document-viewport').evaluate(host=>host.scrollTop)).toBe(350);
 const origin=await view();await button('Bookmarks').click();await page.getByRole('treeitem',{name:/Ending/}).click();await expect(button('Go to page')).toHaveText('Page 4 of 4');
 const ending=await view();await button('Zoom in').click();const zoomed={...ending,zoom:100};await expectView(zoomed);
 await back();await expectView(origin);await expect(button('Undo')).toBeDisabled();await expect(page.getByLabel('Unsaved changes')).toHaveCount(0);
 await page.keyboard.press('Alt+ArrowRight');await expectView(zoomed);await page.keyboard.press('Alt+ArrowLeft');await expectView(origin);
 checks.push('bookmark and Alt history restore reading point/zoom without dirty or edit undo');
 await jump(2);await expect(button('Forward to next view')).toBeDisabled();const second=await view();
 if(desktop){
  await button('Comment').click();await page.locator(`[data-page-id="${second.id}"] .page-canvas`).click({position:{x:80,y:300}});
  await page.getByRole('textbox',{name:'Comment',exact:true}).fill(reviewText);await button('Select').click();
 }
 await button(`Page 2: ${reviewText}`).click();await expect.poll(async()=>(await view()).scrollTop).not.toBe(second.scrollTop);await back();await expectView(second);
 checks.push('page-jump branches and same-page review jump returns');
 await button('Pages').click();await page.getByRole('button',{name:'Page 3 of 4',exact:true}).click();await expect(button('Go to page')).toHaveText('Page 3 of 4');await back();await expectView(second);
 checks.push('thumbnail jumps participate');
 await button('Find').click();const beforeFind=await view();await page.getByRole('searchbox').fill('Needle');await expect(page.getByText('1 of 4',{exact:true})).toBeVisible();
 await page.locator('.document-viewport').evaluate(host=>{host.scrollTop+=100;});const afterFindScroll=await view();
 await button('Settings').click();await button('Done').click();await expectView(afterFindScroll);
 await button('Open').click();if(desktop)await openDialog('Other fixture.pdf');await expect(page.getByRole('tab',{name:'Other fixture.pdf',exact:true})).toHaveAttribute('aria-selected','true');await expect(button('Back to previous view')).toBeDisabled();
 await page.getByRole('tab',{name:'Navigation fixture.pdf',exact:true}).click();await expectView(afterFindScroll);await back();await expect(page.getByRole('searchbox')).toHaveCount(0);await expectView(beforeFind);
 checks.push('Find/modal/tab return preserves scrolled position; history closes Find');
 // Record a destination, move the origin page, and return by stable page identity.
 await jump(1);const beforeReorder=await view();await jump(4);await button('Move up').click();await back();await expectView(beforeReorder);
 checks.push('page reorder retains saved page identity');
 await button('Settings').click();await page.getByRole('combobox',{name:'Page view'}).selectOption('single');await button('Done').click();
 await page.locator('.document-viewport').evaluate(host=>{host.scrollTop=280;});const single=await view();await jump(2);await back();await expectView(single);
 checks.push('single-page reading position return');
 await button('Find').click();await page.getByRole('searchbox').fill('Needle');await expect(page.getByText('1 of 4',{exact:true})).toBeVisible();
 const followed=await view();await page.locator('.document-viewport').evaluate(host=>{host.scrollTop+=150;});
 await page.getByRole('searchbox').fill('Needlex');await expect(page.getByText('No matches',{exact:true})).toBeVisible();
 await page.getByRole('searchbox').fill('Needle');await expect(page.getByText('1 of 4',{exact:true})).toBeVisible();await expectView(followed);
 checks.push('re-entered query navigates after no-match edit');
 await button('Close search').click();await button('Settings').click();await page.getByRole('combobox',{name:'Page view'}).selectOption('continuous');await button('Done').click();
 if(desktop){await button('Open').click();await openDialog('Short fixture.pdf');await expect(page.getByRole('tab',{name:'Short fixture.pdf',exact:true})).toHaveAttribute('aria-selected','true');}
 else await page.goto('http://127.0.0.1:1437/tests/fixtures/navigation-history.html?short');
 await button('Pages').click();await button('Page 1 of 4').click();await expect(button('Back to previous view')).toBeDisabled();
 await button('Find').click();await button('Find').focus();await page.keyboard.press('Alt+ArrowLeft');await expect(page.getByRole('searchbox')).toBeVisible();
 await button('Close search').click();await button('Draw').click();await page.keyboard.press('Alt+ArrowRight');await expect(button('Draw')).toHaveClass(/active/);
 checks.push('short-page no-op and unavailable shortcuts preserve Find/tool');
 const clipped=await page.locator('.page-nav button').evaluateAll(buttons=>buttons.filter(b=>{const r=b.getBoundingClientRect();return r.left<0||r.right>innerWidth||r.top<0||r.bottom>innerHeight;}).map(b=>b.getAttribute('aria-label')));expect(clipped).toEqual([]);
 await page.screenshot({path:resolve(directory,'navigation-900.png')});expect(errors).toEqual([]);
 for(const [name,hash] of Object.entries(hashes))expect(createHash('sha256').update(readFileSync(resolve(directory,name))).digest('hex')).toBe(hash);
 writeFileSync(resolve(directory,'results.json'),JSON.stringify({passed:true,desktop,executableSha256:record?.sha256,checks,origin,zoomed,hashes,errors},null,2));console.log('PASS navigation history acceptance: '+checks.join('; '));
}catch(error){await page.screenshot({path:resolve(directory,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
