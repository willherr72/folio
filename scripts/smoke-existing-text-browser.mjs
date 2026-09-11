import {chromium,expect} from "@playwright/test";
import {mkdirSync,writeFileSync} from "node:fs";
const origin=process.env.FOLIO_TEXT_EDIT_ORIGIN||"http://127.0.0.1:1430";
const browser=await chromium.launch({headless:true});
try{
 const page=await browser.newPage({viewport:{width:1100,height:1000}}),errors=[],checks=[];page.on("pageerror",e=>errors.push(e.message));
 for(const intrinsic of [0,90,180,270])for(const rotation of [0,90,180,270])for(const zoom of [90,170]){
  await page.goto(`${origin}/tests/fixtures/existing-text.html?intrinsic=${intrinsic}&rotation=${rotation}&zoom=${zoom}`);
  const target=page.locator('[data-text-object="0"]');await expect(target).toBeVisible();await target.click();
  await expect(page.getByRole("dialog",{name:"Edit existing text"})).toBeVisible();
  const input=page.getByRole("textbox",{name:"Replacement text"});await expect(input).toBeFocused();
  expect(await input.evaluate(e=>[e.selectionStart,e.selectionEnd])).toEqual([0,14]);await input.fill("New words");await input.press("Enter");
  await expect(page.getByRole("dialog")).toHaveCount(0);await expect(page.locator('[data-pdf-character]')).toHaveCount(9);
  expect(await page.locator('.pdf-text-layer').textContent()).toBe("New words");
  const copied=await page.evaluate(()=>{const spans=document.querySelectorAll('[data-pdf-character]'),range=document.createRange();range.setStart(spans[0].firstChild,0);range.setEnd(spans[spans.length-1].firstChild,1);const selection=getSelection();selection.removeAllRanges();selection.addRange(range);const data=new DataTransfer();document.dispatchEvent(new ClipboardEvent('copy',{clipboardData:data,bubbles:true,cancelable:true}));return data.getData('text/plain');});expect(copied).toBe("New words");checks.push({intrinsic,rotation,zoom});
 }
 expect(errors).toEqual([]);mkdirSync('artifacts',{recursive:true});writeFileSync('artifacts/existing-text-browser.json',JSON.stringify({passed:true,checks,errors,method:'Actual pointer click and keyboard submit at32source/editor/zoomcombinations; changed source text layer and programmatic copy event verified.'},null,2));console.log(`Passed ${checks.length} existing-text rotation/zoom checks`);
}finally{await browser.close();}
