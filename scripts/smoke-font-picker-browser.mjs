import { chromium, expect } from '@playwright/test';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const origin = process.env.FOLIO_FONT_PICKER_ORIGIN || 'http://127.0.0.1:1432';
const browser = await chromium.launch({ headless: true });
const checks = [], errors = [];
mkdirSync('artifacts', { recursive: true });
try {
  for (const theme of ['light', 'dark']) for (const viewport of [{ width: 1280, height: 820 }, { width: 900, height: 600 }, { width: 900, height: 420 }, { width: 390, height: 740 }]) {
    const page = await browser.newPage({ viewport });
    page.on('pageerror', error => errors.push(error.message));
    await page.route('**/__font_picker_fonts__/**', route => {
      const name = decodeURIComponent(new URL(route.request().url()).pathname.split('/').pop()).replace('.ttf', '');
      const filename = { Arial: 'arial.ttf', 'Arial Bold': 'arialbd.ttf', Calibri: 'calibri.ttf', 'Cambria Bold': 'cambriab.ttf', Georgia: 'georgia.ttf', 'Segoe UI': 'segoeui.ttf', 'Times New Roman': 'times.ttf' }[name];
      return route.fulfill({ contentType: 'font/ttf', body: readFileSync(path.join(process.env.WINDIR || 'C:/Windows', 'Fonts', filename)) });
    });
    await page.goto(`${origin}/tests/fixtures/font-picker.html?theme=${theme}${viewport.width < 1000 ? '&long=1' : ''}`);
    await page.getByRole('button', { name: 'More fonts…' }).click();
    await expect(page.getByRole('searchbox', { name: 'Search fonts' })).toBeFocused();
    await expect(page.getByRole('button', { name: 'Apply font', exact: true })).toBeDisabled();
    expect(await page.evaluate(() => window.fontPickerLog.bytes)).toEqual([]);
    await page.keyboard.press('Tab');
    await expect(page.getByRole('button', { name: 'Arial', exact: true })).toBeFocused();
    await page.keyboard.press('ArrowDown');
    await expect(page.getByRole('button', { name: 'Arial Bold', exact: true })).toBeFocused();
    await page.keyboard.press('End');
    await expect(page.getByRole('button', { name: 'Times New Roman', exact: true })).toBeFocused();
    await page.keyboard.press('Home');
    await expect(page.getByRole('button', { name: 'Arial', exact: true })).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(page.getByLabel('Font preview')).toHaveCSS('font-family', 'FolioFont_' + '1'.repeat(64));
    await page.keyboard.press('Tab');
    // Chromium includes an overflowing preview in its keyboard scroll stops.
    if (await page.locator('.font-picker-preview-card').evaluate(node => node === document.activeElement)) await page.keyboard.press('Tab');
    await expect(page.getByRole('button', { name: 'Import font…', exact: true })).toBeFocused();
    await page.getByRole('button', { name: 'DejaVu Serif', exact: true }).click();
    const preview = page.getByLabel('Font preview');
    await expect(preview).toContainText(viewport.width < 1000 ? 'A thoughtful choice' : 'A considered choice');
    await expect(preview).toHaveCSS('font-family', 'FolioFont_' + '5'.repeat(64));
    await expect(preview).toHaveCSS('font-kerning', 'none');
    await expect(preview).toHaveCSS('font-variant-ligatures', 'none');
    await expect(page.getByLabel('Applied font')).toHaveText('None');
    const fontReady = await preview.evaluate(node => document.fonts.check(`25px ${getComputedStyle(node).fontFamily}`, node.textContent));
    expect(fontReady).toBe(true);
    const layout = await page.getByRole('dialog').evaluate(node => {
      const dialog = node.getBoundingClientRect(), footer = node.querySelector('.dialog-actions').getBoundingClientRect();
      const body = node.querySelector('.font-picker-body');
      return { dialog: { x: dialog.x, y: dialog.y, width: dialog.width, height: dialog.height }, footer: { bottom: footer.bottom, right: footer.right }, bodyOverflow: body.scrollWidth > body.clientWidth, viewportWidth: innerWidth, viewportHeight: innerHeight };
    });
    expect(layout.dialog.x).toBeGreaterThanOrEqual(0);
    expect(layout.dialog.y).toBeGreaterThanOrEqual(0);
    if (viewport.width >= 900) expect(layout.dialog.width).toBe(720);
    expect(layout.footer.bottom).toBeLessThanOrEqual(viewport.height);
    expect(layout.footer.right).toBeLessThanOrEqual(viewport.width);
    expect(layout.bodyOverflow).toBe(false);
    await page.screenshot({ path: `artifacts/font-picker-${theme}-${viewport.width}x${viewport.height}.png` });
    await page.getByRole('button', { name: 'Apply font', exact: true }).click();
    await expect(page.getByLabel('Applied font')).toHaveText('DejaVu Serif');
    await expect(page.getByRole('button', { name: 'More fonts…' })).toBeFocused();
    expect(await page.evaluate(() => window.fontPickerLog.released)).toEqual(['1'.repeat(64)]);
    await page.getByRole('button', { name: 'More fonts…' }).click();
    await expect(page.getByLabel('Font preview')).toHaveCSS('font-family', 'FolioFont_' + '5'.repeat(64));
    await page.keyboard.press('Escape');
    await expect(page.getByRole('button', { name: 'More fonts…' })).toBeFocused();
    expect(await page.evaluate(() => window.fontPickerLog.released)).toEqual(['1'.repeat(64)]);
    checks.push({ theme, viewport, layout, fontReady });
    await page.close();
  }
  expect(errors).toEqual([]);
  writeFileSync('artifacts/font-picker-browser.json', JSON.stringify({ passed: true, checks, errors }, null, 2));
  console.log(`PASS font picker: ${checks.length} light/dark viewport combinations, keyboard navigation, exact font preview and Apply/Cancel ownership`);
} finally { await browser.close(); }
