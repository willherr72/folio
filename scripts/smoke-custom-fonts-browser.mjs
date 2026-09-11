import { chromium, expect } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
const browser = await chromium.launch({ headless: true });
const origin = process.env.FOLIO_FONT_ORIGIN || 'http://127.0.0.1:1431';
try {
 const page = await browser.newPage({viewport:{width:1400,height:1100}}), checks=[], errors=[];
 page.on('pageerror', error=>errors.push(error.message));
 for (const rotation of [0,90,180,270]) for (const pageRotation of [0,90,180,270]) {
  await page.goto(`${origin}/tests/fixtures/custom-fonts.html?textRotation=${rotation}&pageRotation=${pageRotation}&zoom=130`);
  const text = page.locator('.document-page [data-overlay] text');
  await expect(text).toHaveAttribute('font-family', 'FolioFont_'+'c'.repeat(64));
  await expect(text).toContainText('AB café Ω WWW iii');
  await expect(page.getByRole('combobox',{name:'Font'})).toHaveValue('custom');
  const sizes=await text.evaluate(node=>({advance:node.getComputedTextLength(),box:(()=>{const b=node.closest('[data-overlay]').querySelector('.selection-box').getBBox();return {width:b.width,height:b.height};})(),faces:[...document.fonts].map(face=>face.family)}));
  const extent=rotation%180?sizes.box.height:sizes.box.width;
  expect(Math.abs(extent-(sizes.advance+8))).toBeLessThan(.6);
  expect(sizes.faces).toEqual(['FolioFont_'+'c'.repeat(64)]);
  await expect(page.locator('aside .thumbnail-canvas text')).toHaveAttribute('font-family','FolioFont_'+'c'.repeat(64));
  const state=JSON.parse(await page.getByLabel('Text state').textContent());expect(state.fontId).toBe('c'.repeat(64));
  checks.push({rotation,pageRotation,...sizes});
 }
 await page.screenshot({path:'artifacts/custom-font-browser.png'});
 expect(errors).toEqual([]);mkdirSync('artifacts',{recursive:true});writeFileSync('artifacts/custom-font-browser.json',JSON.stringify({passed:true,checks,errors},null,2));
 console.log(`Passed ${checks.length} custom-font/page rotation combinations`);
} finally {await browser.close();}
