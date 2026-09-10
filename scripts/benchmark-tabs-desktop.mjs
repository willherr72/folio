import {chromium,expect} from '@playwright/test';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {mkdirSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
const exec=promisify(execFile);
const pid=process.env.FOLIO_APP_PID;
if(!pid)throw Error('Set FOLIO_APP_PID to the isolated test instance.');
const label=process.env.FOLIO_PERF_LABEL||'native';
const output=resolve('artifacts/tab-performance');mkdirSync(output,{recursive:true});
function makePdf(path){
 const lines=Array.from({length:20},(_,row)=>'BT /F1 10 Tf 1 0 0 1 40 '+(740-row*20)+' Tm ('+'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwx'+') Tj ET').join('\n');
 const objects=['<< /Type /Catalog /Pages 2 0 R >>','<< /Type /Pages /Kids [3 0 R] /Count 1 >>','<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>','<< /Length '+Buffer.byteLength(lines)+' >>\nstream\n'+lines+'\nendstream','<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>'];
 let pdf='%PDF-1.7\n';const offsets=[0];
 objects.forEach((object,index)=>{offsets.push(Buffer.byteLength(pdf));pdf+=(index+1)+' 0 obj\n'+object+'\nendobj\n';});
 const xref=Buffer.byteLength(pdf);pdf+='xref\n0 6\n0000000000 65535 f \n'+offsets.slice(1).map(n=>String(n).padStart(10,'0')+' 00000 n \n').join('')+'trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n'+xref+'\n%%EOF\n';
 writeFileSync(path,pdf);
}
const names=['Performance A.pdf','Performance B.pdf'];
for(const name of names)makePdf(resolve(output,name));
const browser=await chromium.connectOverCDP(process.env.FOLIO_CDP_URL||'http://127.0.0.1:9228');
try{
 const page=browser.contexts()[0].pages().find(p=>!p.url().startsWith('devtools:'));
 const cdp=await page.context().newCDPSession(page);await cdp.send('Performance.enable');
 for(const [index,name] of names.entries()){
  await page.getByRole('button',{name:index===0?'Open a PDF':'Open',exact:true}).click();
  await exec('pwsh',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('scripts/set-native-dialog.ps1'),'-AppProcessId',pid,'-FilePath',resolve(output,name),'-Action','Open'],{windowsHide:true,timeout:60000});
  await expect(page.getByRole('tab',{name,exact:true})).toHaveAttribute('aria-selected','true');
  await expect.poll(()=>page.locator('[data-pdf-character]').count(),{timeout:30000}).toBeGreaterThan(1000);
  await expect(page.locator('.page-canvas image')).toHaveCount(1);
 }
 await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
 await page.evaluate(()=>{
  const original=window.__TAURI_INTERNALS__.invoke;
  window.tabRequests={render:0,text:0};
  window.__TAURI_INTERNALS__.invoke=function(command,...args){
   if(command==='render_page')window.tabRequests.render++;
   if(command==='page_text')window.tabRequests.text++;
   return original.call(this,command,...args);
  };
 });
 const metrics=async()=>Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m=>[m.name,m.value]));
 const before=await metrics(),samples=[];
 for(const name of [names[0],names[1],names[0],names[1]]){
  samples.push(await page.evaluate(async name=>{
   const start=performance.now();
   [...document.querySelectorAll('[role=tab]')].find(el=>el.getAttribute('aria-label')===name).click();
   await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
   return performance.now()-start;
  },name));
  await expect(page.locator('.page-canvas image')).toHaveCount(1);
  await expect.poll(()=>page.locator('[data-pdf-character]').count()).toBeGreaterThan(1000);
 }
 const after=await metrics();
 const result={label,charactersPerPage:await page.locator('[data-pdf-character]').count(),switchMs:samples,meanSwitchMs:samples.reduce((a,b)=>a+b)/samples.length,layouts:after.LayoutCount-before.LayoutCount,...await page.evaluate(()=>window.tabRequests)};
 writeFileSync(resolve(output,label+'.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
 if(process.env.FOLIO_PERF_ASSERT==='1'){expect(result.render).toBe(0);expect(result.layouts).toBeLessThan(100);}
 const closed=page.waitForEvent('close');
 await exec('powershell',['-NoProfile','-ExecutionPolicy','Bypass','-File',resolve('scripts/request-native-close.ps1'),'-AppProcessId',pid],{windowsHide:true});
 await closed;
}finally{await browser.close();}
