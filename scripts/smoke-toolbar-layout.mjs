// Real App with synthetic native services. No user PDFs or native writes.
import {chromium,expect} from '@playwright/test';
import {mkdirSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
const baseline=process.argv.includes('--baseline');
const root=resolve('artifacts/toolbar-layout');mkdirSync(root,{recursive:true});
const browser=await chromium.launch({headless:true});
const page=await browser.newPage(),errors=[],results=[];
page.on('pageerror',error=>errors.push(error.message));
try {
 for(const config of [{width:900,height:600,reader:false,dark:false},{width:900,height:600,reader:true,dark:true},{width:1280,height:820,reader:true,dark:false},{width:1920,height:1080,reader:false,dark:true},...(!baseline?[{width:853,height:547,reader:true,dark:true,zoomEquivalent:"1280x820 at 150%"}]:[])]) {
  await page.setViewportSize({width:config.width,height:config.height});
  await page.goto(`http://127.0.0.1:1437/tests/fixtures/toolbar.html?${config.reader?'reader&':''}${config.dark?'dark':''}`);
  await expect(page.getByRole('toolbar')).toBeVisible();
  const geometry=await page.evaluate(()=>{
   const toolbar=document.querySelector('.toolbar'),workspace=document.querySelector('.workspace');
   const buttons=[...toolbar.querySelectorAll('button')].map(button=>{const r=button.getBoundingClientRect();return {name:button.getAttribute('aria-label')||button.textContent,left:r.left,right:r.right,top:r.top,bottom:r.bottom,disabled:button.disabled};});
   return {width:innerWidth,toolbarWidth:toolbar.clientWidth,toolbarScrollWidth:toolbar.scrollWidth,bodyWidth:document.body.scrollWidth,workspaceHeight:workspace.getBoundingClientRect().height,buttons,clipped:buttons.filter(b=>b.left<0||b.right>innerWidth||b.top<0||b.bottom>innerHeight)};
  });
  results.push({config,...geometry});
  await page.screenshot({path:resolve(root,`${baseline?'before':'after'}-${config.width}-${config.reader?'reader':'owner'}.png`)});
  if(!baseline){
   expect(geometry.clipped).toEqual([]);expect(geometry.toolbarScrollWidth).toBeLessThanOrEqual(geometry.toolbarWidth);expect(geometry.bodyWidth).toBeLessThanOrEqual(config.width);expect(geometry.workspaceHeight).toBeGreaterThan(config.zoomEquivalent?290:350);
   const focusable=page.getByRole("toolbar").locator("button:not([disabled])");
   await focusable.first().focus();
   for(let index=0;index<await focusable.count();index++){await expect(focusable.nth(index)).toBeFocused();await page.keyboard.press("Tab");}
   await expect(page.getByRole('button',{name:'Edit text',exact:true}))[config.reader?'toBeDisabled':'toBeEnabled']();
   await expect(page.getByRole('button',{name:'Insert image',exact:true}))[config.reader?'toBeDisabled':'toBeEnabled']();
   const save=page.getByRole('button',{name:'Save options',exact:true});await save.focus();await page.keyboard.press('Enter');
   await expect(page.getByRole('menuitem',{name:'Save editable PDF',exact:true})).toBeFocused();
   const menu=await page.getByRole('menu').boundingBox(),trigger=await save.boundingBox();
   expect(menu.y).toBeCloseTo(trigger.y+trigger.height+8);expect(menu.x+menu.width).toBeLessThanOrEqual(config.width);
   await page.keyboard.press('Escape');await expect(save).toBeFocused();await expect(page.getByRole('menu')).toHaveCount(0);
   await page.keyboard.press('Tab');await expect(page.getByRole('button',{name:config.reader?'Unlock editing':'Select',exact:true})).toBeFocused();
   await page.getByRole('button',{name:'Find',exact:true}).focus();await page.keyboard.press('Enter');await expect(page.getByRole('searchbox',{name:'Find in document'})).toBeVisible();await page.keyboard.press('Escape');
   await page.keyboard.press('Control+f');await expect(page.getByRole('searchbox',{name:'Find in document'})).toBeFocused();await page.keyboard.press('Escape');
   await page.getByRole('button',{name:'Zoom in',exact:true}).click();await expect(page.locator('.zoom-value')).toHaveText('100%');
  }
 }
 expect(errors).toEqual([]);
 writeFileSync(resolve(root,baseline?'baseline.json':'results.json'),JSON.stringify({passed:!baseline,results,errors},null,2));
 console.log(JSON.stringify({baseline,layouts:results.map(r=>({config:r.config,clipped:r.clipped.map(b=>b.name),toolbarScrollWidth:r.toolbarScrollWidth,toolbarWidth:r.toolbarWidth,workspaceHeight:r.workspaceHeight}))},null,2));
}catch(error){await page.screenshot({path:resolve(root,'failure.png')}).catch(()=>{});throw error;}finally{await browser.close();}
