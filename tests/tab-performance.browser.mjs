import {chromium,expect} from '@playwright/test';
import {writeFileSync,mkdirSync} from 'node:fs';
const characters=Number(process.env.FOLIO_PERF_CHARACTERS || 1000);
const browser=await chromium.launch({headless:true});
try{
 const page=await browser.newPage({viewport:{width:1440,height:1050}});
 const cdp=await page.context().newCDPSession(page);
 await cdp.send('Performance.enable');
 await page.goto((process.env.FOLIO_PERF_ORIGIN || "http://127.0.0.1:1423")+"/tests/fixtures/tab-performance.html?characters="+characters);
 await page.getByRole('button',{name:'Open a PDF',exact:true}).click();
 await expect(page.locator('[data-pdf-character]')).toHaveCount(characters);
 await page.getByRole('button',{name:'Open',exact:true}).click();
 await expect(page.getByRole('tab',{name:'PDF2.pdf'})).toHaveAttribute('aria-selected','true');
 await expect(page.locator('[data-pdf-character]')).toHaveCount(characters);
 await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
 const baseline=await page.evaluate(()=>({...window.tabPerf}));
 const metrics=async()=>Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m=>[m.name,m.value]));
 const before=await metrics(),samples=[];
 for(const name of ['PDF1.pdf','PDF2.pdf','PDF1.pdf','PDF2.pdf']){
  samples.push(await page.evaluate(async name=>{
   const start=performance.now();
   [...document.querySelectorAll('[role=tab]')].find(el=>el.getAttribute('aria-label')===name).click();
   await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
   return performance.now()-start;
  },name));
  await expect(page.locator('[data-pdf-character]')).toHaveCount(characters);
  await expect(page.locator('.page-canvas image')).toHaveCount(1);
 }
 const after=await metrics(),stats=await page.evaluate(()=>window.tabPerf);
 const result={charactersPerPage:characters,switchMs:samples,meanSwitchMs:samples.reduce((a,b)=>a+b)/samples.length,layouts:after.LayoutCount-before.LayoutCount,styleRecalculations:after.RecalcStyleCount-before.RecalcStyleCount,repeatRenderRequests:stats.renders-baseline.renders,repeatTextRequests:stats.textRequests-baseline.textRequests};
 console.log(JSON.stringify(result,null,2));
 mkdirSync('artifacts/tab-performance',{recursive:true});
 writeFileSync('artifacts/tab-performance/'+(process.env.FOLIO_PERF_LABEL||'current')+'.json',JSON.stringify(result,null,2));
 if(process.env.FOLIO_PERF_ASSERT==='1'){
  expect(result.repeatRenderRequests).toBe(0);
  expect(result.layouts).toBeLessThan(100);
 }
}finally{await browser.close();}
