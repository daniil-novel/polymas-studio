import { expect, test } from '@playwright/test';
import { instanceSnapshot } from './fixtures/instances';

for (const width of [736, 1280, 1600]) {
  test(`5000 instance definitions remain navigable at ${width}px`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.route('**/api/**', async (route) => {
      expect(route.request().method()).toBe('GET');
      const path = new URL(route.request().url()).pathname;
      if (path === '/api/events')
        return route.fulfill({ json: { events: [], cursor: 0, head: 0 } });
      expect(path).toBe('/api/state');
      return route.fulfill({ json: instanceSnapshot() });
    });
    await page.goto('/');
    await page.getByRole('button', { name: /^Агенты \d/ }).click();
    await page.getByRole('button', { name: /Экземпляры/ }).click();
    const table = page.getByRole('table', { name: 'Экземпляры агентов' });
    await expect(table).toHaveAttribute('aria-rowcount', '5000');
    expect(await table.getByRole('row').count()).toBeLessThan(40);
    const viewport = page.getByRole('region', { name: 'Прокрутка экземпляров' });
    await viewport.focus();
    await page.keyboard.press('Control+End');
    await expect(table).toContainText('Исследователь · 4999');
    expect(await table.getByRole('row').count()).toBeLessThan(40);
    await page.getByLabel('Поиск экземпляров').fill('agent-directory-4998');
    await expect(table).toContainText('Выполняет задачу');
    await expect(table).toContainText('fixture-worker');
    expect(await viewport.evaluate((element) => element.scrollTop)).toBe(0);
    await page.getByLabel('Поиск экземпляров').fill('');
    await page.getByRole('button', { name: 'Следующие строки' }).click();
    expect(await viewport.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
    await page.getByRole('button', { name: 'Предыдущие строки' }).click();
    expect(await viewport.evaluate((element) => element.scrollTop)).toBe(0);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(
      true,
    );
    const box = await viewport.boundingBox();
    expect(box!.width).toBeGreaterThanOrEqual(380);
    expect(box!.x + box!.width).toBeLessThanOrEqual(width);
    expect(errors).toEqual([]);
    await page.screenshot({ path: testInfo.outputPath('instances.png'), fullPage: true });
  });
}
