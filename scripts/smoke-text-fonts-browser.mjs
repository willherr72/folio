import { chromium, expect } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
const browser = await chromium.launch({ headless: true });
const origin = process.env.FOLIO_FONT_ORIGIN || 'http://127.0.0.1:1431';
try {
  const page = await browser.newPage({ viewport: { width: 1400, height: 1100 } }), checks = [], errors = [];
  page.on('pageerror', error => errors.push(error.message));
  for (const rotation of [0, 90, 180, 270]) {
    await page.goto(`${origin}/tests/fixtures/text-fonts.html?textRotation=${rotation}&rotation=90&zoom=170`);
    const picker = page.getByRole('combobox', { name: 'Font' });
    const fonts = await picker.locator('option').evaluateAll(nodes => nodes.map(node => node.value));
    expect(fonts).toHaveLength(12);
    for (const font of fonts) {
      await picker.selectOption(font);
      await page.getByRole('textbox', { name: 'Content' }).fill('AB wide WWW iii office fi fl');
      const text = page.locator('.document-page [data-overlay] text');
      await expect(text).toHaveAttribute('font-weight', font.includes('Bold') ? '700' : '400');
      await expect(text).toHaveAttribute('font-style', /Italic|Oblique/.test(font) ? 'italic' : 'normal');
      await expect(page.locator('aside .thumbnail-canvas text')).toHaveAttribute('font-family', await text.getAttribute('font-family'));
      // The selection must follow actual browser advances, not the old fixed .56em estimate.
      const sizes = await text.evaluate(node => ({ advance: node.getComputedTextLength(), box: (() => { const b = node.closest('[data-overlay]').querySelector('.selection-box').getBBox(); return {width:b.width,height:b.height}; })() }));
      const extent = rotation % 180 ? sizes.box.height : sizes.box.width;
      expect(Math.abs(extent - (sizes.advance + 8))).toBeLessThan(.6);
      checks.push({ font, rotation, advance: sizes.advance, selectionExtent: extent });
    }
  }
  expect(errors).toEqual([]);
  mkdirSync('artifacts', { recursive: true });
  writeFileSync('artifacts/text-fonts-browser.json', JSON.stringify({ passed: true, checks, errors }, null, 2));
  console.log(`Passed ${checks.length} font and rotation checks`);
} finally { await browser.close(); }
